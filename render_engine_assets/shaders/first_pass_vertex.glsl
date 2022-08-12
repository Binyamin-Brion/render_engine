void main()
{
    for(int i = 0; i < numberLightMatrices; i++)
    {
        lightFragPos[i] = lightMatrices[i] * translation * vec4(aPos, 1.0);
    }

    textureLayer = layers;
    textureCoords = texCoords;

    useSkyboxTexture = renderingSkybox;
    skyBoxTexCoords = aPos;

    drawingLightSource = renderingLightSource;

    cameraPosition = cameraLocation;

    adjustBrightnessLightSource = lightSource;

    if (renderingSkybox == 1)
    {
      normalizedVertexNormal = normalize(normal);
        vec4 pos = projectionMatrix * viewMatrix * vec4(aPos, 1.0);
        fragPosition = vec3(translation * vec4(aPos, 1.0));
        fragPosition = pos.xyz;
        gl_Position = pos.xyww;
    }
    else
    {
        if(drawOutline == 1)
        {
            vec3 modPos = aPos * 1.1;
            gl_Position = projectionMatrix * viewMatrix * translation * vec4(modPos , 1.0);
            normalizedVertexNormal = normalize(vec3(translation * vec4(normal, 0.0)));
        }
        else
        {
            vec3 modPos = aPos * 1;
            gl_Position = projectionMatrix * viewMatrix * translation * vec4(modPos , 1.0);
            normalizedVertexNormal = normalize(vec3(translation * vec4(normal, 0.0)));
        }

           fragPosition = vec3(translation * vec4(aPos, 1.0));
    }
}