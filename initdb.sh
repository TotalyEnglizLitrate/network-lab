#!/usr/bin/env bash

source .env

docker compose up -d database
DATABASE=$(docker ps -q --filter "name=database")
if [ -z "$DATABASE" ]; then
    echo "Error: database container failed to start"
    exit 1
fi
docker exec -i "${DATABASE}" psql -U $POSTGRES_USER -c "CREATE DATABASE guacamole_db;"
docker run --rm guacamole/guacamole /opt/guacamole/bin/initdb.sh --postgresql | \
    docker exec -i "${DATABASE}" psql -U $POSTGRES_USER -d guacamole_db -f -
