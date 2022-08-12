const vec3 SKY_BOX_FRAG = vec3(1.0 / 0.0, 0.0, 0.0);
const vec3 LIGHT_SOURCE_FRAG = vec3(0.0, 1.0 / 0.0, 0.0);

uniform vec3 skyboxBrightness;
uniform uint drawingModelsWithTextures;

struct TextureInformation
{
    uint array_index;
    uint index_offset;
};


TextureInformation diffuse_texture_info(uvec4 tex_info)
{
    return TextureInformation( (tex_info.x & uint(0xFC00)) >> 10, tex_info.x & uint(0x3FF) );
}

TextureInformation dissolve_texture_info(uvec4 tex_info)
{
    return TextureInformation( tex_info.x >> uint(26), (tex_info.x >> 16) & uint(0x3FF) );
}

TextureInformation normal_texture_info(uvec4 tex_info)
{
    return TextureInformation( (tex_info.y & uint(0xFC00)) >> 10, tex_info.y & uint(0x3FF) );
}

TextureInformation shininess_texture_info(uvec4 tex_info)
{
    return TextureInformation( tex_info.y >> uint(26), (tex_info.y >> 16) & uint(0x3FF) );
}

TextureInformation specular_texture_info(uvec4 tex_info)
{
    return TextureInformation( (tex_info.z & uint(0xFC00)) >> 10, tex_info.z & uint(0x3FF) );
}

vec4 textureColour()
{
    if(drawingModelsWithTextures == 0)
    {
        return textureCoords;
    }
    else if(useSkyboxTexture == 1)
    {
        vec4 skyBoxColour = vec4(texture(skyBox, skyBoxTexCoords));
        skyBoxColour.r *= skyboxBrightness.r;
        skyBoxColour.g *= skyboxBrightness.g;
        skyBoxColour.b *= skyboxBrightness.b;
        return skyBoxColour;
    }
    else
    {
        float brightnessAdjustment = adjustBrightnessLightSource == 1 ? 2.0 : 1.0;

        vec2 scaledTexCoords = vec2(textureCoords.x * textureCoords.z, textureCoords.y * textureCoords.w);
        TextureInformation textureLocation = diffuse_texture_info(textureLayer);

        switch(textureLocation.array_index)
        {
            case 0:
                return vec4(texture(errorTextureArray, vec3(scaledTexCoords, textureLocation.index_offset))) * brightnessAdjustment;

            case 1:
                return vec4(texture(textureArray, vec3(scaledTexCoords, textureLocation.index_offset))) * brightnessAdjustment;

            case 2:
                return vec4(texture(solidColour, vec3(scaledTexCoords, textureLocation.index_offset))) * brightnessAdjustment;

            default:
                return vec4(0.7, 0.0, 0.0, 0.0);
        }
    }
}

void main()
{
    // store the fragment position vector in the first gbuffer texture
    gPosition = useSkyboxTexture == 1 ? SKY_BOX_FRAG : drawingLightSource == 1 ? LIGHT_SOURCE_FRAG : fragPosition;
    // also store the per-fragment normals into the gbuffer
    gNormal = normalize(normalizedVertexNormal);
    // and the diffuse per-fragment color
    gAlbedoSpec = textureColour();

    gLightPosition = lightFragPos[2];
}