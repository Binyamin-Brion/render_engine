void main()
{
    gl_Position = projectionMatrix * viewMatrix * translation * vec4(aPos, 1.0);
}