#!/bin/bash
set -e
docker-compose stop node1 bitmaskd
docker system prune -f
docker volume prune -f
