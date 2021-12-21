#!/bin/bash
changed=0;
git pull | grep "Already up" || changed=1
#echo "ch: $changed"
if [ $changed -eq 1 ]; then
  echo "Rebuilding"
  podman build -f tourcalc.run.docker -t tourcalc:latest --no-cache .
  podman stop tourcalc
  podman rm tourcalc
  podman run -d -p 127.0.0.1:8080:80 --env-file ~/config/tourcalc.env --name tourcalc localhost/tourcalc:latest
fi

if [ $changed -eq 0 ]; then
  echo "NO news. Do not rebuild"
fi

