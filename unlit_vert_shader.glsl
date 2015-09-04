#version 120

in vec2 pos;
in vec2 texcoord;

uniform mat4 modelViewMatrix;
uniform mat4 projMatrix;

varying vec2 Texcoord;

void main() {
  gl_Position = projMatrix * modelViewMatrix * vec4(pos, 0.0, 1.0);
  Texcoord = texcoord;
}
