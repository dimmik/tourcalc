#!/bin/bash
changed=0;
git pull | grep "Already up" || changed=1
#echo "ch: $changed"
if [ $changed -eq 1 ]; then
  echo "Rebuilding"
  podman build -f tourcalc.run.docker -t tourcalc:latest --no-cache .
fi

if [ $changed -eq 0 ]; then
  echo "NO news. Do not rebuild"
fi

