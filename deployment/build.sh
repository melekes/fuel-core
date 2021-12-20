#!/bin/sh

NAME=ghcr.io/fuellabs/fuel-core
TAG=latest

docker build -t ${NAME}:${TAG} --ssh default -f deployment/Dockerfile .
docker save ${NAME}:${TAG} | gzip > deployment/image.tar.gz
