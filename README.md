# CodePush Server (Bun + SQLite Edition)

![Docker Image Size (tag)](https://img.shields.io/docker/image-size/quockhanh2924/code-push-server/latest)
![Bun](https://img.shields.io/badge/Runtime-Bun-black?logo=bun)
![SQLite](https://img.shields.io/badge/Database-SQLite-blue?logo=sqlite)

A lightweight, high-performance CodePush server implementation. This is a fork of the original project, optimized to run on **Bun** and **SQLite**, significantly reducing RAM usage and simplifying deployment.

## Key Features & Changes

-   **High Performance:** Migrated from Node.js to [Bun](https://bun.sh/) runtime.
-   **Lightweight:** Uses **SQLite** by default. No heavy MySQL container required (saving ~300MB+ RAM).
-   **Docker Ready:** Optimized multi-stage Dockerfile based on Alpine.
-   **Compatibility:** Fully compatible with the official React Native CodePush client and CLI.
-   **Storage Support:** Local, Qiniu, S3, OSS, Tencent Cloud.

## Quick Start with Docker (Recommended)

This is the fastest way to get started. You only need Docker and Docker Compose.

### 1. Create `docker-compose.yml`

```yaml
version: '3.7'
services:
  server:
    image: ghcr.io/khanh29204/code-push-server:latest
    container_name: code-push-server
    restart: always
    volumes:
      - ./data/storage:/data/storage   # Store binary bundles
      - ./data/database:/data/tmp      # Store SQLite database file
    environment:
      # Server Config
      PORT: 3000
      HOST: '0.0.0.0'
      # Security (CHANGE THIS!)
      TOKEN_SECRET: 'INSERT_YOUR_RANDOM_SECRET_KEY_HERE'
      # Public URL for App to download bundles (Replace with your domain/IP)
      DOWNLOAD_URL: 'http://YOUR_PUBLIC_IP:3000/download'
      # Database Config (SQLite)
      DB_DIALECT: 'sqlite'
      DB_STORAGE_FILE: '/data/tmp/codepush.sqlite'
      # Storage Config (Local)
      STORAGE_TYPE: 'local'
      STORAGE_DIR: '/data/storage'
      # Redis (Required for session/locking)
      REDIS_HOST: 'redis'
      REDIS_PORT: 6379
    ports:
      - '3000:3000'
    depends_on:
      - redis

  redis:
    image: redis:alpine
    container_name: code-push-redis
    restart: always
    volumes:
      - ./data/redis:/data

# If you need to migrate data from old MySQL, see Migration Guide below.
```

### 2. Run the server

```bash
docker-compose up -d
```

### 3. Verification

-   **Admin URL:** `http://localhost:3000`
-   **Default Account:**
    -   User: `admin`
    -   Password: `123456`

> **Note:** The server will automatically create the SQLite database and seed the default admin account on the first run.

---

## Configuration

You can configure the server using environment variables in `docker-compose.yml`.

| Variable | Description | Default |
| :--- | :--- | :--- |
| `DB_DIALECT` | Database type (`sqlite`, `mysql`, `postgres`) | `sqlite` |
| `DB_STORAGE_FILE` | Path to SQLite file (if using sqlite) | `/data/tmp/codepush.sqlite` |
| `DOWNLOAD_URL` | Public URL where apps download bundles | `http://127.0.0.1:3000/download` |
| `TOKEN_SECRET` | Secret key for JWT (Important!) | `INSERT_RANDOM_TOKEN_KEY` |
| `STORAGE_TYPE` | Storage backend (`local`, `s3`, `oss`, `qiniu`) | `local` |
| `STORAGE_DIR` | Directory for local storage | `/data/storage` |
| `LOG_LEVEL` | Logging level (`debug`, `info`, `error`) | `info` |

---

## Development / Manual Installation

If you want to run it without Docker, ensure you have **Bun** installed.

1.  **Install Bun:**
    ```bash
    curl -fsSL https://bun.sh/install | bash
    ```

2.  **Install Dependencies:**
    ```bash
    bun install
    ```

3.  **Build:**
    ```bash
    bun run build
    ```

4.  **Initialize Database:**
    ```bash
    bun bin/db.js init
    ```

5.  **Start Server:**
    ```bash
    bun bin/www.js
    ```

---

## Migration from MySQL to SQLite

If you are migrating from an older version using MySQL:

1.  **Stop the new server:** `docker-compose down`
2.  **Convert Data:** Use a tool like [mysql-to-sqlite3](https://github.com/techouse/mysql-to-sqlite3) to convert your MySQL dump to a SQLite file.
    ```bash
    # Example command
    npx mysql-to-sqlite3 -h YOUR_OLD_MYSQL_HOST -u user -p -d codepush -f codepush.sqlite
    ```
3.  **Reset Sequences (Important):** SQLite auto-increment might get out of sync after import. Open the `codepush.sqlite` file and execute:
    ```sql
    DELETE FROM sqlite_sequence;
    INSERT INTO sqlite_sequence (name, seq) SELECT 'users', MAX(id) FROM users;
    INSERT INTO sqlite_sequence (name, seq) SELECT 'apps', MAX(id) FROM apps;
    -- Repeat for all tables with ID ...
    ```
4.  **Upload:** Copy the `codepush.sqlite` file to your server's mapped volume folder (`./data/database` in the example above).
5.  **Start Server:** `docker-compose up -d`

---

## Supported Storage Backends

-   **Local:** Stores bundles on the server disk (default).
-   **Qiniu:** Seven ox cloud storage.
-   **S3:** AWS S3 or compatible (MinIO, DigitalOcean Spaces).
-   **OSS:** Aliyun OSS.
-   **Tencent Cloud:** COS.

To configure S3/OSS/etc, add the relevant keys to `environment` variables (e.g., `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, etc. - check `src/core/config.ts` for mapping).

## CLI Usage

Use the official CodePush CLI or [code-push-cli](https://github.com/shm-open/code-push-cli) to interact with this server.

```bash
npm install -g code-push-cli
code-push login http://YOUR_SERVER_IP:3000
```