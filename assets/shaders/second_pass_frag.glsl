const float SPECIAL_FRAG_VALUE = 1.0 / 0.0;

in vec2 textureCoords;

vec3 calculateDirectionLights();
vec3 calculateSpotLights(vec3 fragPosition, vec3 normalDirection, vec3 diffuseColour, vec4 lightFragPosition);
vec3 calculatePointLights();


vec3 calculateAmbient(vec4 ambientColour);
vec3 calculateDiffuse(vec3 lightDirection, vec3 diffuseColour);
vec3 calculateSpecular(vec3 lightDirection, vec3 specularColour, vec3 specularTextureColour, float specularFactor);
float calculateAttenuation(float linear, float quadratic, vec3 lightPosition);

float shadowCalculation(vec3 lightDirection, vec4 lightFragPosition, vec3 normalDirection)
{
    float bias = max(0.005 * (1.0 - dot(normalDirection, lightDirection)), 0.00001);

    for(int as = 2; as < 3; as++)
    {
        vec3 projCoords = lightFragPosition.xyz / lightFragPosition.w;

        projCoords = projCoords * 0.5 + 0.5;

        float closestDepth = texture(shadowMaps, vec3(projCoords.xy, 2)).r;

        float currentDepth = projCoords.z - 0.00005;

        if(projCoords.z > 1.0)
        {
            continue;
        }

        if(projCoords.x < 0 || projCoords.x > 1 || projCoords.y < 0 || projCoords.y > 1)
        {
            continue;
        }

        if(currentDepth < closestDepth && currentDepth > 0.01)
        {
            return 1.0;
        }
        else
        {
            /*
                Consider following scenario:    |
                                             __x|

                The x is at a corner of a texture. If x and surrounding texels has a depth value of 0.3, then x should have
                a filtered value of 0.3. However, since x is at corner, three depth values will be from texture border colour,
                which is 1.0, Thus x's depth will now be (4 * 0.3 + 1.0 * 3) / 9 = ~0.45 . To avoid this issue, surrounding
                texels are clamped to valid texture coordinates, resulting in x's coordinates.

                The depth value of x might not be the same as the neighbouring frag if it could be sampled from same shadow map, but
                it is likely to be closer than a constant 1.0. Even if 1.0 is in fact closer to the real neighbouring depth value,
                clamping will simply reduce the effective number of texels for filtering, which is not ideal but is still correct.
            */

            float shadow = 0.0;
            vec2 texelSize = 1.0 / textureSize(shadowMaps, 0).xy;

            for(int x = -1; x < 2; ++x)
            {
                for(int y = -1; y < 2; ++y)
                {
                    float xCoord = clamp((projCoords.x + y) * texelSize.x, 0.0, 1.0);
                    float yCoord = clamp((projCoords.y + y) * texelSize.y, 0.0, 1.0);

                   float pcfDepth = texture(shadowMaps, vec3(xCoord, yCoord, 2)).r;
                   shadow += currentDepth - bias > pcfDepth  ? 1.0 : 0.0;
                }
            }

            // If shadow maps aren't available for whatever reason, a light should still influence the scene.
            // If the return value is 0 then it won't
            return max(shadow / 9.0, 0.5);
        }
    }

    return 1.0;
}

void main()
{
    if(anyLightSourceVisible == 0)
    {
        FragColor = vec4(calculateAmbient(vec4(1.0, 1.0, 1.0, 0.5)), 1.0);
    }
    else
    {
       vec3 fragPosition = texture(gPosition, vec3(textureCoords, 0)).rgb;

        if(fragPosition.x == SPECIAL_FRAG_VALUE)
        {
             FragColor = vec4(calculateAmbient(vec4(1.0, 1.0, 1.0, 0.5)), 1.0);
        }
        else if(fragPosition.y == SPECIAL_FRAG_VALUE)
        {
            FragColor = texture(gAlbedoSpec, vec3(textureCoords, 0));
        }
        else
        {
            vec3 normalDirection = texture(gNormal, vec3(textureCoords, 0)).rgb;
            vec3 diffuseColour = texture(gAlbedoSpec, vec3(textureCoords, 0)).rgb;
            vec4 lightFragPosition = texture(gLightPosition, vec3(textureCoords, 0));

            vec3 lightColour = calculateSpotLights(fragPosition, normalDirection, diffuseColour, lightFragPosition);
            lightColour.r = clamp(lightColour.r, 0.0, 1.0);
            lightColour.g = clamp(lightColour.g, 0.0, 1.0);
            lightColour.b = clamp(lightColour.b, 0.0, 1.0);

            FragColor = vec4(lightColour, 1.0);
        }
    }
}

vec3 calculateSpotLights(vec3 fragPosition, vec3 normalDirection, vec3 diffuseColour, vec4 lightFragPosition)
{
    vec3 lightColour = vec3(0.0, 0.0, 0.0);

    for(int i = 0; i < numberSpotLights; ++i)
    {
        vec3 lightDir2 = normalize(spotLightPositions[i] - texture(gPosition, vec3(textureCoords, 0)).rgb);
        float shadowValue = shadowCalculation(lightDir2, lightFragPosition, normalDirection);

        float attenuation = calculateAttenuation(spotLightLinearCoefficient[i], spotLightQuadraticCoefficient[i], spotLightPositions[i]);
        lightColour += calculateAmbient(spotLightAmbientColour[i]);

        // *** Diffuse ***

        float diffuse_coefficient = max(dot(normalDirection, lightDir2), 0.0);
        lightColour += spotLightDiffuseColour[i] * diffuseColour * diffuse_coefficient * shadowValue;

        // *** Specular ***

      /*  vec3 cameraDir = normalize(cameraPosition - texture(gPosition, vec3(textureCoords, 0)).rgb);
        vec3 halfwayDir = normalize(lightDirection + cameraDir);
        float specular = pow(max(dot(texture(gNormal, vec3(textureCoords, 0)).rgb, halfwayDir), 0.0), specularFactor) * shadowValue;
        return specularColour * specular * vec3(1.0, 1.0, 1.0); */

        lightColour += calculateSpecular(lightDir2, spotLightSpecularColour[i], vec3(1.0, 1.0, 1.0), 64.0) * attenuation;
    }

    return lightColour;
}

vec3 calculateDirectionLights()
{
    vec3 lightColour = vec3(0.0, 0.0, 0.0);

    for(int i = 0; i < numberDirectionLights; ++i)
    {
        lightColour += calculateAmbient(directionLightAmbientColour[i]);
        lightColour += calculateDiffuse(-directionLightDir[i], directionLightDiffuseColour[i]);

        lightColour += calculateSpecular(-directionLightDir[i], directionLightSpecularColour[i], vec3(1.0, 1.0, 1.0), 64.0);
    }

    return lightColour;
}

vec3 calculatePointLights()
{
    vec3 lightColour = vec3(0.0, 0.0, 0.0);

    for(int i = 0; i < numberPointLights; ++i)
    {
        float angleFragLight = dot((normalize(texture(gPosition, vec3(textureCoords, 0)).rgb) - pointLightPositions[i]), normalize(pointLightDirections[i]));
        float epsilon = cutOff[i] - outerCutoff[i];
        float intensity = clamp((angleFragLight - outerCutoff[i]) / epsilon, 0.0, 1.0);

        vec3 lightDir2 = normalize(pointLightPositions[i] - texture(gPosition, vec3(textureCoords, 0)).rgb);

        float attenuation = calculateAttenuation(pointLightLinearCoefficient[i], pointLightQuadraticCoefficient[i], pointLightPositions[i]);
        lightColour += calculateAmbient(pointLightAmbientColour[i]);
        lightColour += calculateDiffuse(lightDir2, pointLightDiffuseColour[i]) * attenuation * intensity;
        lightColour += calculateSpecular(lightDir2, pointLightSpecularColour[i], vec3(1.0, 1.0, 1.0), 64.0) * attenuation;
    }

    return lightColour;
}

vec3 calculateAmbient(vec4 ambientColour)
{
    return texture(gAlbedoSpec, vec3(textureCoords, 0)).rgb * ambientColour.rgb * ambientColour.a;
}

vec3 calculateDiffuse(vec3 lightDirection, vec3 diffuseLightColour)
{
    float diffuse_coefficient = max(dot(texture(gNormal, vec3(textureCoords, 0)).rgb, lightDirection), 0.0);
    return diffuseLightColour * texture(gAlbedoSpec, vec3(textureCoords, 0)).rgb * diffuse_coefficient;
}

vec3 calculateSpecular(vec3 lightDirection, vec3 specularColour, vec3 specularTextureColour, float specularFactor)
{
    vec3 cameraDir = normalize(cameraPosition - texture(gPosition, vec3(textureCoords, 0)).rgb);
    vec3 halfwayDir = normalize(lightDirection + cameraDir);
    float specular = pow(max(dot(texture(gNormal, vec3(textureCoords, 0)).rgb, halfwayDir), 0.0), specularFactor);
    return specularColour * specular * vec3(1.0, 1.0, 1.0);
}

float calculateAttenuation(float linear, float quadratic, vec3 lightPosition)
{
    float distanceToFrag = length(lightPosition - texture(gPosition, vec3(textureCoords, 0)).rgb);
    return 1.0 / (1.0 + linear * distanceToFrag + quadratic * distanceToFrag * distanceToFrag);
}