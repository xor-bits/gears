#!/bin/bash
script_path=$(dirname "$0")

for filename in ${script_path}/*.glsl; do
	stage=${filename:$(expr ${#filename} - 9):4}
    glslc -fshader-stage=${stage} "${filename}" -o "${filename}.spv"
done
