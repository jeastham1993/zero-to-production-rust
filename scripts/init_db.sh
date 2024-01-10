#!/usr/bin/env bash

set -x
set -eo pipefail

DB_USER="${POSTGRES_USER:=postgres}"
DB_PASSWORD="${POSTGRES_PASSWORD:=password}"
DB_NAME="${POSTGRES_DB:=newsletter}"
DB_HOST="${POSTGRES_HOST:=localhost}"
DB_PORT="${POSTGRES_PORT:=5432}"

# Skip Docker run if Postgres already running
if [[ -z "${SKIP_DOCKER}" ]]
then
  docker run \
    -e POSTGRES_USER=${DB_USER} \
    -e POSTGRES_PASSWORD=${DB_PASSWORD} \
    -e POSTGRES_DB=${DB_NAME} \
    -p "${DB_PORT}":5432 \
    -d postgres \
    postgres -N 1000
fi

export PGPASSWORD="${DB_PASSWORD}"

until psql -h "${DB_HOST}" -U "${DB_USER}" -p "${DB_PORT}" -d "postgres" -c '\q'; do
  >&2 echo "Postgres is still unavailable - sleeping"
  sleep 1
done

DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}
export DATABASE_URL

>&2 echo "Postgres is up and running on port ${DB_PORT} - running migrations now!"

sqlx database create
sqlx migrate run

>&2 echo "Postgres migrated. Ready to go! :-)"

>&2 echo "Starting Jaeger for local OpenTelemetry tracing"

docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest

>&2 echo "Jaeger started, access the UI at http://localhost:16686"

>&2 echo "Starting DynamoDB Local"

docker run -d -p 8000:8000 amazon/dynamodb-local:latest

>&2 echo "DynamoDB Local Started, creating table"

aws dynamodb create-table \
    --table-name newsletter \
    --attribute-definitions AttributeName=PK,AttributeType=S AttributeName=SK,AttributeType=S AttributeName=GSI1PK,AttributeType=S AttributeName=GSI1SK,AttributeType=S  \
    --key-schema AttributeName=PK,KeyType=HASH AttributeName=SK,KeyType=RANGE \
    --provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5 \
    --endpoint-url http://localhost:8000 \
    --region us-east-1 \
    --global-secondary-indexes \
            "[
                {
                    \"IndexName\": \"GSI1\",
                    \"KeySchema\": [
                        {\"AttributeName\":\"GSI1PK\",\"KeyType\":\"HASH\"},
                        {\"AttributeName\":\"GSI1SK\",\"KeyType\":\"RANGE\"}
                    ],
                    \"Projection\": {
                        \"ProjectionType\":\"ALL\"
                    },
                    \"ProvisionedThroughput\": {
                        \"ReadCapacityUnits\": 5,
                        \"WriteCapacityUnits\": 5
                    }
                }
            ]"

sleep 2

>&2 echo "Table created"