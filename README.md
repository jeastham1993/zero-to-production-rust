# Zero to Production Rust - The Serverless One

As an aspiring Rust developer, I found an incredible amount of value in [Luca Palmieri](https://twitter.com/algo_luca) book Zero to Production Rust. Frankly, it's one of the best programming books I've ever read, let alone Rust ones.

After finishing the book, I decided to take the final application and make it run using completely serverless technologies. The system in the book is a single process application, that interacts with a Postgres database and uses Redis as a session store. The serverless version, looks something like this...

![](./assets/zero2prod-serverless-architecture.png)