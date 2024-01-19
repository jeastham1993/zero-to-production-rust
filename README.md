# Zero to Production Rust - The Serverless One

As an aspiring Rust developer, I found an incredible amount of value in [Luca Palmieri](https://twitter.com/algo_luca) book Zero to Production Rust. Frankly, it's one of the best programming books I've ever read, let alone Rust ones.

After finishing the book, I decided to take the final application and make it run using completely serverless technologies. The system in the book is a single process application, that interacts with a Postgres database and uses Redis as a session store. The serverless version, looks something like this...

![](./assets/zero2prod-serverless-architecture.png)

## Distributed Tracing

The application is fully OpenTelemetry compatible, currently configured to export trace data to Jaeger when running locally and to Honeycomb when running in AWS. OpenTelemetry configuration is found in the [telemetry.rs](./src/api/src/telemetry.rs). When running inside Lambda, trace data is flushed using the `force_flush()` function after every request is processed. You can see an example using [Axum Middleware](./src/api/src/middleware.rs) or as part of a [Lambda function handler](./src/backend/src/bin/lambda/send_confirmation.rs). The backend handlers processing the DynamoDB stream also support trace propagation, to continue a trace from the API call through to the backend process.

## Test

To test locally first ensure you have Docker up and running. Then:

1. Execute script under [src/api/scripts/init_db.sh](./src/api/scripts/init_db.sh). This starts local Docker containers and creates DynamoDB tables in DynamoDB Local
2. Execute `cargo test` in either the [api](./src/api) or [backend](./src/backend/) folder to run tests
3. Run `cargo run` in the [api](./src/api) folder to startup the Axum application locally


## Local Run

When you start the application up for the first time, make a GET request to `/util/_migrate`. This will create the initial admin user, with a password of `James!23`. **IMPORTANT! If deploying to AWS ensure you immediately login and change the admin user password**

Once the migrate endpoint is executed, you can interact with the API

*TODO! Add API endpoint examples*

## Deploy

The API and backend are both deployed together in a single CDK stack. This is to simplify deployment. In production, this would split into 2 separate stacks for 2 separate microservices.

## Future Development

- [ ] Introduce EventBridge Pipes to decouple DynamoDB stream from backend processors
- [ ] Introduce SQS to improve durability
- [ ] Implement StepFunctions to manage email sending, to iterate over list of subscribers
- [ ] Add CICD pipelines to demonstrate CICD best practices
