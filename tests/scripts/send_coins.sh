#!/bin/bash
set -e
docker-compose exec -T node1 cli sendtoaddress $1 $2
docker-compose exec -T node1 cli -generate 2
sleep 3
