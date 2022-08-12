void main()
{
    for(int i = 0; i < numberLightMatrices; i++)
    {
        lightFragPos[i] = lightMatrices[i] * translation * vec4(aPos, 1.0);
    }

    textureLayer = layers;
    textureCoords = texCoords;

    useSkyboxTexture = renderingSkybox == 1 ? 1.0 : 0.0;
    skyBoxTexCoords = aPos;

    fragPosition = vec3(translation * vec4(aPos, 1.0));
    normalizedVertexNormal = normalize(normal);

    cameraPosition = cameraLocation;

    if (renderingSkybox == 1)
    {
        vec4 pos = projectionMatrix * viewMatrix * vec4(aPos, 1.0);
        gl_Position = pos.xyww;
    }
    else
    {
        if(drawOutline == 1)
        {
            vec3 modPos = aPos * 1.1;
            gl_Position = projectionMatrix * viewMatrix * translation * vec4(modPos , 1.0);
        }
        else
        {
            vec3 modPos = aPos * 1;
            gl_Position = projectionMatrix * viewMatrix * translation * vec4(modPos , 1.0);
        }
    }

    // gl_Position = projection * view * translation * vec4(aPos * 0.5, 1.0);
}