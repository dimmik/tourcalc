#!/bin/sh
podman build -f tourcalc.run.docker -t tourcalc:latest --no-cache .
podman stop tourcalc
podman rm tourcalc
podman run -d -p 127.0.0.1:8080:80 --env-file ~/config/tourcalc.env --name tourcalc localhost/tourcalc:latest 
