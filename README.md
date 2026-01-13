# mill-valley-sausalito-bikepath-flood-alert

This is the source code for the [website](https://mv-sausalito-bike-flood-forecast.bandoro.dev) which makes it easy to see and get notified about flooding on the Mill Valley to Sausalito bike path in Marin County, California.

## Local Development
The webserver can be run locally for development by following the instructions below:

Install sqlx cli for Sqlite:
```shell
cargo install sqlx-cli --no-default-features --features sqlite
```

Create the database and run migrations:
```shell
export DATABASE_URL=sqlite:data/flood.db

sqlx database create
sqlx migrate run
```

Create a `.env` file based on the `.env.sample-dev` file and fill in the required environment variables. If you want to test the SMTP email sending functionality, you will need to provide valid SMTP server credentials.

Run the webserver:
```shell
cargo run -- serve
```
To see flood predictions, the following command will fetch and update flood data:
```shell
cargo run -- sync
```

## Deployment
The application is automatically deployed using a self hosted runner on Raspberry Pi. The current deployment requires a .env file with `TUNNEL_TOKEN` set to run behind a Cloudflare tunnel.



