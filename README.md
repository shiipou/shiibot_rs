# shiibot_rs

A Rust Discord bot to manage temporary voice channels using Serenity and Poise.

## Features

- **Lobby Voice Channel**: Create a lobby channel with the `/create_lobby` slash command
- **Automatic Temp Channels**: When a user joins the lobby, a temporary voice channel is automatically created for them
- **Permission Inheritance**: Temporary channels inherit permissions from the lobby channel that created them
- **Auto-cleanup**: Temporary channels are automatically deleted when empty
- **Channel Configuration**: Users can rename their temporary channel using an interactive form
- **Persistent Storage**: Uses PostgreSQL database to persist state across bot restarts

## Setup

1. Create a Discord application at [Discord Developer Portal](https://discord.com/developers/applications)
2. Create a bot user and get the bot token
3. Enable the following intents in the bot settings:
   - Server Members Intent
   - Message Content Intent (optional)

4. Invite the bot to your server with the following permissions:
   - Manage Channels
   - Move Members
   - Send Messages
   - Use Slash Commands
   - View Channels

5. That will give you a link like that :  
   `https://discord.com/api/oauth2/authorize?client_id=505756999351533579&permissions=277042170896&scope=bot%20applications.commands`  
   Invite your bot in your server

6. Set up a PostgreSQL database and get the connection URL

## Running the Bot

Set the required environment variables and run the bot:

```bash
export DISCORD_TOKEN="your_bot_token_here"
export DATABASE_URL="postgres://user:password@localhost/shiibot"
cargo run
```

Or create a `.env` file in the project root:

```env
DISCORD_TOKEN=your_bot_token_here
```

Then run using docker compose (recommanded):

```bash
docker compose up --build
```

Then press `w` to enable auto-reload on changes.

## Database

The bot uses PostgreSQL to store:
- Lobby channels (so they persist across restarts)
- Temporary channels (to track ownership and continue managing them after restart)

The database tables are created automatically on startup.

### Required Tables (auto-created)

```sql
CREATE TABLE lobby_channels (
    channel_id BIGINT PRIMARY KEY,
    guild_id BIGINT NOT NULL
);

CREATE TABLE temp_channels (
    channel_id BIGINT PRIMARY KEY,
    guild_id BIGINT NOT NULL,
    owner_id BIGINT NOT NULL,
    lobby_channel_id BIGINT NOT NULL
);
```

## Commands

### `/create_lobby`

Creates a lobby voice channel. When users join this channel, they will be moved to their own temporary voice channel.

**Parameters:**
- `name` (optional): Custom name for the lobby channel. Defaults to "➕ Create Voice Channel"

**Required Permissions:** Manage Channels

## How It Works

1. An admin uses `/create_lobby` to create a lobby voice channel
2. When a user joins the lobby channel:
   - A new temporary voice channel is created with the user's name
   - The channel inherits all permissions from the lobby channel
   - The user gets additional management permissions (move, mute, deafen members)
   - The user is automatically moved to their new channel
   - A configuration message is sent with a button to customize the channel
3. The channel owner can click "Configure Channel" to rename their channel
4. When all users leave a temporary channel, it is automatically deleted

## Requirements

- Rust 1.70+ (2021 edition)
- PostgreSQL database
- Discord bot token with required permissions

## Docker

This project uses [Dofigen](https://github.com/lenra-io/dofigen) for Docker image generation, extending from [dofigen-hub](https://github.com/lenra-io/dofigen-hub).

### Building with Dofigen

1. Install Dofigen:
   ```bash
   cargo install dofigen
   ```

2. Generate Dockerfile and build:
   ```bash
   dofigen gen
   docker build -t shiibot_rs .
   ```

### Running with Docker

```bash
docker run -e DISCORD_TOKEN=your_token -e DATABASE_URL=postgres://user:password@host/db shiibot_rs
```

Or with Docker Compose:

```yaml
version: '3.8'
services:
  bot:
    build: .
    environment:
      - DISCORD_TOKEN=${DISCORD_TOKEN}
      - DATABASE_URL=postgres://postgres:password@db/shiibot
    depends_on:
      - db
  db:
    image: postgres:15
    environment:
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB=shiibot
    volumes:
      - pgdata:/var/lib/postgresql/data

volumes:
  pgdata:
```
