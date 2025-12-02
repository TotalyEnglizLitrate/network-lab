#!/usr/bin/env bash

source .env

DATABASE=$(docker ps -q --filter "name=database")
docker compose up -d database
docker exec -i "${DATABASE}" psql -U $POSTGRES_USER -c "CREATE DATABASE guacamole_db;"
docker run --rm guacamole/guacamole /opt/guacamole/bin/initdb.sh --postgresql | \
    docker exec -i "${DATABASE}" psql -U $POSTGRES_USER -d guacamole_db -f -
