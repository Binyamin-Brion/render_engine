in flat uint intRenderingLightVolumes;
in vec2 textureCoords;

// ***** Begin function declarations *****

// *** Light type functions ***
vec3 calculateDirectionLights(vec3 fragPosition, vec3 objectNormal, vec3 objectDiffuse);
vec3 calculatePointLights(vec3 fragPosition, vec3 objectNormal, vec3 objectDiffuse);
vec3 calculateSpotLights(vec3 fragPosition, vec3 objectNormal, vec3 objectDiffuse, vec4 lightFragPosition);

// *** Light calculation functions ***
vec3 calculateAmbient(vec3 objectDiffuse, vec4 ambientColour);
vec3 calculateDiffuse(vec3 lightDirection, vec3 lightDiffuse, vec3 objectNormal, vec3 objectDiffuse);
vec3 calculateSpecular(vec3 fragPosition, vec3 lightDirection, vec3 lightSpecular, vec3 objectNormal, float specularFactor);
float calculateAttenuation(vec3 fragPosition, float linear, float quadratic, vec3 lightPosition);
float shadowCalculation(vec3 lightDirection, vec4 lightFragPosition, vec3 objectNormal);

// ***** End function declarations ******

void main()
{
    if(intRenderingLightVolumes == 1)
    {

    }
    else if(renderSkybox == 1)
    {
        FragColor = texture(gAlbedoSpec, vec3(textureCoords, 0));
    }
    else if(anyLightSourceVisible == 0)
    {
        vec3 objectDiffuse = texture(gAlbedoSpec, vec3(textureCoords, 0)).rgb;
        FragColor = vec4(calculateAmbient(objectDiffuse, vec4(1.0, 1.0, 1.0, defaultDiffuseFactor)), 1.0);
    }
    else
    {
        vec3 fragPosition = texture(gPosition, vec3(textureCoords, 0)).rgb;
        vec3 objectNormal = texture(gNormal, vec3(textureCoords, 0)).rgb;
        vec3 objectDiffuse = texture(gAlbedoSpec, vec3(textureCoords, 0)).rgb;
        vec4 lightFragPosition = texture(gLightPosition, vec3(textureCoords, 0));

        vec3 lightColour = calculateSpotLights(fragPosition, objectNormal, objectDiffuse, lightFragPosition);
        lightColour += calculatePointLights(fragPosition, objectNormal, objectDiffuse);
        lightColour += calculateSpotLights(fragPosition, objectNormal, objectDiffuse, lightFragPosition);

        lightColour.r += int(lightColour.r < noLightSourceCutoff) * objectDiffuse.r * defaultDiffuseFactor;
        lightColour.g += int(lightColour.g < noLightSourceCutoff) * objectDiffuse.g * defaultDiffuseFactor;
        lightColour.b += int(lightColour.b < noLightSourceCutoff) * objectDiffuse.b * defaultDiffuseFactor;

        lightColour.r = clamp(lightColour.r, 0.0, 1.0);
        lightColour.g = clamp(lightColour.g, 0.0, 1.0);
        lightColour.b = clamp(lightColour.b, 0.0, 1.0);

        FragColor = vec4(lightColour, 1.0);
    }
}

vec3 calculateDirectionLights(vec3 fragPosition, vec3 objectNormal, vec3 objectDiffuse)
{
    vec3 lightColour = vec3(0.0, 0.0, 0.0);

    for(int i = 0; i < numberDirectionLights; ++i)
    {
        lightColour += calculateAmbient(objectDiffuse, directionLightAmbientColour[i]);
        lightColour += calculateDiffuse(-directionLightDirection[i], directionLightDiffuseColour[i], objectNormal, objectDiffuse);
        lightColour += calculateSpecular(fragPosition, -directionLightDirection[i], directionLightSpecularColour[i], objectNormal, 64.0);
    }

    return lightColour;
}

vec3 calculatePointLights(vec3 fragPosition, vec3 objectNormal, vec3 objectDiffuse)
{
    vec3 lightColour = vec3(0.0, 0.0, 0.0);

    for(int i = 0; i < numberPointLights; ++i)
    {
        float angleFragLight = dot((normalize(fragPosition) - pointLightPosition[i]), normalize(pointLightDirection[i]));
        float epsilon = cutOff[i] - outerCutoff[i];
        float intensity = clamp((angleFragLight - outerCutoff[i]) / epsilon, 0.0, 1.0);

        vec3 negativeLightDirection = normalize(pointLightPosition[i] - fragPosition);

        float attenuation = calculateAttenuation(fragPosition, pointLightLinearCoefficient[i], pointLightQuadraticCoefficient[i], pointLightPosition[i]);
        lightColour += calculateAmbient(objectDiffuse, pointLightAmbientColour[i]) * attenuation;
        lightColour += calculateDiffuse(negativeLightDirection, pointLightDiffuseColour[i], objectNormal, objectDiffuse) * attenuation * intensity;
        lightColour += calculateSpecular(fragPosition, negativeLightDirection, pointLightSpecularColour[i], objectNormal, 64.0) * attenuation;
    }

    return lightColour;
}

vec3 calculateSpotLights(vec3 fragPosition, vec3 objectNormal, vec3 objectDiffuse, vec4 lightFragPosition)
{
    vec3 lightColour = vec3(0.0, 0.0, 0.0);

    for(int i = 0; i < numberSpotLights; ++i)
    {
        if(length(spotLightPosition[i] - fragPosition) > spotLightRadius[i])
        {
            continue;
        }

        vec3 negativeLightDirection = normalize(spotLightPosition[i] - fragPosition);
        float shadowValue = shadowCalculation(negativeLightDirection, lightFragPosition, objectNormal);

        float attenuation = calculateAttenuation(fragPosition, spotLightLinearCoefficient[i], spotLightQuadraticCoefficient[i], spotLightPosition[i]);
        lightColour += calculateAmbient(objectDiffuse, spotLightAmbientColour[i]) * attenuation;
        lightColour += calculateDiffuse(negativeLightDirection, spotLightDiffuseColour[i], objectNormal, objectDiffuse) * attenuation;
        lightColour += calculateSpecular(fragPosition, negativeLightDirection, spotLightSpecularColour[i], objectNormal, 64.0) * attenuation;
    }

    return lightColour;
}

vec3 calculateAmbient(vec3 objectDiffuse, vec4 ambientColour)
{
    return objectDiffuse * ambientColour.rgb * ambientColour.a;
}

vec3 calculateDiffuse(vec3 lightDirection, vec3 lightDiffuse, vec3 objectNormal, vec3 objectDiffuse)
{
    float diffuse_coefficient = max(dot(objectNormal, lightDirection), 0.0);
    return lightDiffuse * objectDiffuse * diffuse_coefficient;
}

vec3 calculateSpecular(vec3 fragPosition, vec3 lightDirection, vec3 lightSpecular, vec3 objectNormal, float specularFactor)
{
    vec3 cameraDir = normalize(cameraPosition - fragPosition);
    vec3 halfwayDir = normalize(lightDirection + cameraDir);
    float adjustedSpecularFactor = pow(max(dot(objectNormal, halfwayDir), 0.0), specularFactor);
    return lightSpecular * adjustedSpecularFactor;
}

float calculateAttenuation(vec3 fragPosition, float linear, float quadratic, vec3 lightPosition)
{
    float distanceToFrag = length(lightPosition - fragPosition);
    return 1.0 / (1.0 + linear * distanceToFrag + quadratic * distanceToFrag * distanceToFrag);
}

float shadowCalculation(vec3 lightDirection, vec4 lightFragPosition, vec3 objectNormal)
{
    float bias = max(0.005 * (1.0 - dot(objectNormal, lightDirection)), 0.00001);

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