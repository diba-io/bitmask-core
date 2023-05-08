#!/bin/bash
set -e
docker-compose up -d node1 bitmaskd
sleep 10
docker-compose exec -T node1 cli loadwallet default
docker-compose exec -T node1 cli -generate 500
sleep 5
