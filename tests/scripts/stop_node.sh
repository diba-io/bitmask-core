#!/bin/bash
set -e
docker-compose stop node1
docker system prune -f
docker volume prune -f
