layout (location = 0) in vec3 aPos;
layout (location = 1) in vec2 texCoords;
layout (location = 2) in vec4 lightInformation;

out flat uint intRenderingLightVolumes;
out vec2 textureCoords;

void main()
{
    textureCoords = texCoords;
    intRenderingLightVolumes = renderingLightVolumes;

    if(renderingLightVolumes == 1)
    {
        gl_Position = projViewMatrix * vec4(aPos * lightInformation.w + lightInformation.xyz, 1.0);
    }
    else
    {
         gl_Position = vec4(aPos, 1.0);
    }
}