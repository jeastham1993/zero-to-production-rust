#!/usr/bin/env bash

set -x
set -eo pipefail

>&2 echo "Starting Jaeger for local OpenTelemetry tracing"

docker run -d -e COLLECTOR_OTLP_ENABLED=true -p4317:4317 -p4318:4318 -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest

>&2 echo "Jaeger started, access the UI at http://localhost:16686"

>&2 echo "Starting DynamoDB Local"

docker run -d -p 8000:8000 amazon/dynamodb-local:latest

>&2 echo "DynamoDB Local Started, creating table"

sleep 2

aws dynamodb create-table \
    --table-name newsletter \
    --attribute-definitions AttributeName=PK,AttributeType=S AttributeName=GSI1PK,AttributeType=S AttributeName=GSI1SK,AttributeType=S  \
    --key-schema AttributeName=PK,KeyType=HASH \
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
                        \"ProjectionType\":\"KEYS_ONLY\"
                    },
                    \"ProvisionedThroughput\": {
                        \"ReadCapacityUnits\": 5,
                        \"WriteCapacityUnits\": 5
                    }
                }
            ]" > create-result.json

aws dynamodb create-table \
    --table-name auth \
    --attribute-definitions AttributeName=PK,AttributeType=S  \
    --key-schema AttributeName=PK,KeyType=HASH \
    --provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5 \
    --endpoint-url http://localhost:8000 \
    --region us-east-1  > auth-table-create-result.json

sleep 2

>&2 echo "Table created"

>&2 echo "Setting dummy environment variables for DynamoDB Local"

#export AWS_ACCESS_KEY_ID=AKIAW22GPLBZ36234YRA
#export AWS_SECRET_ACCESS_KEY=local
#export AWS_REGION=us-east-1
# on Mac the below command should be used if you receive a "too many files open error"
# ulimit -n 2000