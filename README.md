# CodePush Server (Rust Edition)

![Rust](https://img.shields.io/badge/Rust-2024-orange?logo=rust)
![Axum](https://img.shields.io/badge/Axum-0.8-blue?logo=tokio)
![Docker Ready](https://img.shields.io/badge/Docker-Ready-blue?logo=docker)
![License](https://img.shields.io/badge/License-MIT-green)
![CI/CD](https://img.shields.io/github/actions/workflow/status/khanh29204/code-push-server/main.yml?branch=master&label=Build%20%26%20Deploy)

Một phiên bản CodePush Server được viết lại hoàn toàn bằng **Rust** (dựa trên web framework [Axum](https://github.com/tokio-rs/axum)), thay thế hoàn hảo cho phiên bản Node.js gốc. Dự án mang lại hiệu năng vượt trội, mức sử dụng tài nguyên CPU/RAM cực thấp, khởi động tức thì và khả năng vận hành ổn định trong môi trường production.

Tương thích 100% với **CodePush CLI** và SDK client ([react-native-code-push](https://github.com/microsoft/react-native-code-push)), đồng thời tích hợp sẵn **Web Admin Dashboard** trực quan phục vụ quản trị hệ thống.

---

## Tính năng nổi bật

- **Giao diện Web Admin Dashboard**: Tích hợp sẵn giao diện quản trị Web hiện đại tại `http://localhost:3000` phục vụ quản lý Apps, Deployments, Releases, Access Keys và Storage Audit mà không bắt buộc gõ lệnh CLI.
- **Tương thích toàn diện**: Hỗ trợ đầy đủ các lệnh CodePush CLI (`login`, `app add`, `release-react`, `deployment ls`, `promote`, `rollback`,...).
- **Cập nhật thông minh (Delta/Diff Updates)**: Tự động tính toán và tạo các gói diff (gói chênh lệch) cho các phiên bản phát hành, giúp giảm dung lượng tải về của client React Native/Cordova.
- **Quản lý Rollout & Mandatory Update**: Hỗ trợ cập nhật bắt buộc và phát hành theo tỷ lệ phần trăm (Staged Rollout) dựa trên Unique ID của thiết bị.
- **Hệ thống Auth & Access Keys**:
  - Đăng nhập, đăng ký bằng mã xác thực Email (SMTP).
  - Giới hạn số lần đăng nhập sai (Rate Limiting qua Redis).
  - Quản lý Access Tokens/Keys linh hoạt với thời gian sống (TTL) tùy chỉnh cho CI/CD hoặc CLI.
- **Kiến trúc dữ liệu tối ưu**:
  - **SQLite** với chế độ WAL (Write-Ahead Logging) cho khả năng ghi/đọc đồng thời cao.
  - Tự động dọn dẹp và checkpoint WAL (`PRAGMA wal_checkpoint(TRUNCATE)`) khi tắt server (Graceful Shutdown).
  - **Redis** cho caching (Update Check, Rollout, Rate Limit).
- **Công cụ Storage Audit**: Endpoint & UI tích hợp sẵn cho phép kiểm tra tính toàn vẹn của tệp tin lưu trữ (phát hiện file rác `orphaned` hoặc tệp tin bị thiếu `missing`).
- **Docker Ready**: Dockerfile multi-stage tạo binary static trên Alpine (`x86_64-unknown-linux-musl`), container runtime cực nhẹ chạy từ `scratch`.

---

## Tech Stack

- **Core & Runtime**: Rust (Edition 2024), [Tokio](https://tokio.rs/) (Async Runtime)
- **Web Framework**: [Axum](https://github.com/tokio-rs/axum) 0.8
- **Database & Query**: [SQLx](https://github.com/launchbadge/sqlx) 0.8 (SQLite), [bb8-redis](https://github.com/khuezy/bb8-redis) / Redis
- **Security & Tokens**: `jsonwebtoken`, `bcrypt`, `sha2`, `sha1`, `md5`
- **Compression & Hash**: `zip`, `walkdir`, thuật toán hash gói QETAG tương thích CodePush
- **Email**: `lettre` (SMTP client)
- **Frontend & Web UI**: HTML5, Tailwind CSS, JavaScript (Vanilla ES6), Feather/Heroicons
- **Static Assets & Logging**: `tower-http`, `tracing`, `tracing-subscriber`

---

## Cấu trúc dự án

```text
code-push-server/
├── Cargo.toml               # Phụ thuộc và cấu hình Rust package
├── Dockerfile               # Dockerfile multi-stage (Musl static -> Scratch)
├── docker-compose.yml       # Cấu hình chạy nhanh với Docker Compose
├── .env.example             # Mẫu tệp cấu hình biến môi trường
├── public/                  # Các file Web UI Admin & Auth tĩnh
│   ├── html/                # Giao diện HTML (index, login, register, tokens, password, confirm)
│   ├── js/                  # Scripts logic quản trị Web UI
│   └── stylesheets/         # Custom CSS styles
├── data/                    # Thư mục mặc định chứa cơ sở dữ liệu SQLite
├── data-storage/            # Thư mục lưu trữ các gói bundle zip phát hành
└── src/
    ├── main.rs              # Entry point: khởi tạo Tracing, Axum Router, Graceful Shutdown
    ├── config/              # Đọc cấu hình biến môi trường (AppConfig)
    ├── core/                # Kết nối DB, SQLite WAL checkpoint, AppError, Hằng số
    ├── models/              # Định nghĩa Structs cho các bảng dữ liệu
    ├── routes/              # Modular Axum Routers
    │   ├── apps/            # Quản lý Ứng dụng & Phát hành (app_handlers, collaborators, deployments, releases)
    │   ├── storage.rs       # Kiểm tra & dọn dẹp Storage Audit (GET /storage/audit, DELETE /storage/audit)
    │   ├── access_keys.rs   # Quản lý Access Keys cho CLI/CI-CD
    │   ├── auth.rs          # Đăng nhập & Xác thực JWT
    │   ├── users.rs         # Đăng ký & Mã xác nhận Email
    │   ├── account.rs       # Thông tin tài khoản
    │   └── index_v1.rs      # SDK Client Update API
    ├── services/            # Logic nghiệp vụ (Account, App, Package, Client, Deployments)
    └── utils/               # Utils (Security, Storage, Zip diff, Qetag, Extractors)
```

---

## Cấu hình biến môi trường (`.env`)

Tạo file `.env` dựa trên `.env.example`:

```bash
cp .env.example .env
```

Chi tiết các biến cấu hình:

| Biến môi trường | Mặc định | Mô tả |
| :--- | :--- | :--- |
| `PORT` | `3000` | Cổng HTTP Server lắng nghe |
| `LOG_LEVEL` | `info` | Cấp độ log (`trace`, `debug`, `info`, `warn`, `error`) |
| `DATABASE_URL` | `sqlite://../data/codepush.db` | Đường dẫn kết nối SQLite |
| `JWT_TOKEN_SECRET` | - | Secret key mã hóa JWT (Chuỗi ngẫu nhiên 64 ký tự) |
| `REDIS_HOST` | `127.0.0.1` | Địa chỉ Redis host |
| `REDIS_PORT` | `6379` | Cổng kết nối Redis |
| `REDIS_PASSWORD` | - | Mật khẩu Redis (nếu có) |
| `REDIS_DB` | `0` | Database index của Redis |
| `STORAGE_TYPE` | `local` | Loại lưu trữ gói bundle (`local`) |
| `STORAGE_DIR` | `../data-storage` | Thư mục vật lý lưu trữ file ZIP trên disk |
| `LOCAL_DOWNLOAD_URL` | `http://127.0.0.1:3000/download` | URL cho client tải gói cập nhật |
| `ALLOW_REGISTRATION` | `true` | Cho phép người dùng mới tự đăng ký tài khoản (`true`/`false`) |
| `TRY_LOGIN_TIMES` | `4` | Số lần đăng nhập sai tối đa trong ngày (0 = không giới hạn) |
| `DIFF_NUMS` | `3` | Số lượng gói diff tối đa tạo cho các phiên bản cũ |
| `DATA_DIR` | `/tmp` | Thư mục tạm dùng để tính toán gói diff |
| `UPDATE_CHECK_CACHE` | `false` | Bật/tắt cache kết quả updateCheck trong Redis (`true`/`false`) |
| `ROLLOUT_CLIENT_UNIQUE_ID_CACHE` | `false` | Bật/tắt cache Unique ID thiết bị rollout trong Redis (`true`/`false`) |
| `SMTP_HOST` | - | Địa chỉ SMTP server gửi email xác nhận |
| `SMTP_PORT` | `465` | Cổng SMTP |
| `SMTP_USERNAME` | - | Tài khoản SMTP |
| `SMTP_PASSWORD` | - | Mật khẩu SMTP |

---

## Giao diện Web Admin Dashboard (`http://localhost:3000`)

Server tích hợp sẵn Web UI trực quan, truy cập trực tiếp tại địa chỉ server (ví dụ: `http://localhost:3000`).

Tài khoản mặc định sau khi khởi tạo CSDL:
- **Tài khoản (Username / Email)**: `admin` (hoặc `admin@codepush.com`)
- **Mật khẩu (Password)**: `123456`

Các tính năng chính:
- **Đăng nhập & Đăng ký**: Truy cập `/login` hoặc `/register` để đăng nhập tài khoản quản trị.
- **Quản lý Apps & Deployments**: Xem danh sách ứng dụng, tạo app mới, quản lý các môi trường (Staging, Production), xem lịch sử release chi tiết.
- **Promote & Rollback trên Web**: Thao tác promote bản phát hành từ Staging lên Production hoặc thực hiện rollback chỉ với một click.
- **Quản lý Access Keys (`/tokens`)**: Tạo và quản lý Access Tokens dành riêng cho CodePush CLI hoặc quy trình CI/CD.
- **Kiểm tra Storage Audit**: Giao diện theo dõi tính toàn vẹn các file ZIP gói cập nhật, phát hiện và xóa file rác (orphaned files) trực tiếp trên Web.
- **Hỗ trợ Đa ngôn ngữ**: Chuyển đổi linh hoạt giữa giao diện Tiếng Việt và Tiếng Anh.

---

## Hướng dẫn cài đặt và chạy

### 1. Chạy trực tiếp với Cargo (Local Development)

**Yêu cầu**: Rust toolchain, Redis server đang chạy.

```bash
# 1. Cài đặt các biến môi trường
cp .env.example .env

# 2. Biên dịch và chạy server
cargo run --release
```

Server sẽ khởi tạo CSDL SQLite tự động tại đường dẫn thiết lập trong `DATABASE_URL` và lắng nghe tại `http://localhost:3000`.

### 2. Chạy với Docker Compose

```bash
# Khởi chạy server container
docker-compose up -d
```

Hoặc build thủ công Docker image:

```bash
docker build -t code-push-server:latest .
docker run -d -p 3000:3000 --env-file .env --name code-push-server code-push-server:latest
```

---

## CI/CD & Automated Deployment (GitHub Actions)

Dự án tích hợp sẵn quy trình CI/CD tự động hóa qua GitHub Actions (`.github/workflows/main.yml`):

- **Lọc đường dẫn thông minh (Smart Path Filtering)**: Chỉ kích hoạt build lại khi có thay đổi thực sự trong mã nguồn (`src/`), file tĩnh (`public/`), phụ thuộc (`Cargo.toml`/`Cargo.lock`), hoặc cấu hình Docker (`Dockerfile`, `docker-compose.yml`, `.dockerignore`). Việc thay đổi tài liệu (`README.md`), cấu hình Postman (`postman.json`), tệp mẫu `.env.example` hay file cấu hình tool/IDE sẽ **không kích hoạt build lại không cần thiết**.
- **Tối ưu hóa Caching & Build Time**: Sử dụng `docker/build-push-action` cùng cơ chế cache GitHub Actions (`cache-from: type=gha`, `cache-to: type=gha,mode=max`) kết hợp multi-stage Docker build, giúp tái sử dụng các dependency layer đã biên dịch để rút ngắn tối đa thời gian build Docker image.
- **Quản lý Concurrency**: Sử dụng cấu hình `concurrency` với `cancel-in-progress: true` để tự động hủy các job build cũ nếu có commit mới được push liên tục lên branch `master`, tránh lãng phí tài nguyên và rủi ro race-condition khi deploy.
- **Tự động Push & Deploy**: Build Docker image và đẩy lên GitHub Container Registry (`ghcr.io`), sau đó tự động kết nối qua SSH vào Production Server để thực thi script deploy (`pull.sh`).

---

## Sử dụng với CodePush CLI

### 1. Đăng nhập qua CodePush CLI

Đặt Management Server URL về địa chỉ server Rust:

```bash
code-push login http://localhost:3000
```
Trình duyệt sẽ mở trang hiển thị Access Key (`/tokens`), copy key và dán vào CLI.

### 2. Quản lý Ứng dụng & Deployment

```bash
# Thêm ứng dụng mới
code-push app add MyApp android react-native
code-push app add MyApp ios react-native

# Xem danh sách ứng dụng
code-push app ls

# Xem danh sách Deployment Key (Staging, Production)
code-push deployment ls MyApp -k
```

### 3. Phát hành bản cập nhật (Release)

```bash
# Phát hành bản cập nhật React Native cho Android
code-push release-react MyApp android -d Staging --des "Cập nhật tính năng mới" --targetBinaryVersion "~1.0.0"

# Phát hành bắt buộc (Mandatory)
code-push release-react MyApp android -d Production -m --des "Bản sửa lỗi khẩn cấp"

# Phát hành theo tỷ lệ 25% người dùng (Rollout)
code-push release-react MyApp android -d Production -r 25
```

### 4. Promote & Rollback

```bash
# Promote bản cập nhật từ Staging lên Production
code-push promote MyApp Staging Production --des "Promote bản test lên Production"

# Rollback bản cập nhật gần nhất trên Production
code-push rollback MyApp Production
```

---

## Danh sách API Endpoints

### Client Endpoints (App / SDK)
- `GET /updateCheck` hoặc `GET /v0.1/public/codepush/update_check`: Client kiểm tra bản cập nhật mới.
- `POST /reportStatus/download` hoặc `POST /v0.1/public/codepush/report_status/download`: Báo cáo tải gói cập nhật thành công.
- `POST /reportStatus/deploy` hoặc `POST /v0.1/public/codepush/report_status/deploy`: Báo cáo trạng thái cài đặt bundle (thành công/thất bại/rollback).

### Management / CLI Endpoints
- `POST /auth/login`: Đăng nhập lấy JWT Token.
- `GET /accessKeys` | `POST /accessKeys` | `DELETE /accessKeys/{name}`: Quản lý Access Keys cho CLI.
- `GET /apps` | `POST /apps` | `DELETE /apps/{app_name}`: Quản lý Apps.
- `GET /apps/{app_name}/deployments`: Danh sách Deployments.
- `POST /apps/{app_name}/deployments/{deployment_name}/release`: Upload bản phát hành mới.
- `POST /apps/{app_name}/deployments/{source}/promote/{dest}`: Promote bản phát hành.
- `POST /apps/{app_name}/deployments/{deployment_name}/rollback`: Rollback bản phát hành.

### System & Maintenance API
- `GET /health`: Health check endpoint (Trả về `OK`).
- `GET /config`: Lấy cấu hình công khai của server.
- `GET /storage/audit`: Kiểm tra tính toàn vẹn của Storage (Yêu cầu Token Auth). Trả về danh sách file hợp lệ, file mồ côi (`orphaned_files`) và file bị thiếu (`missing_files`).
- `DELETE /storage/audit`: Dọn dẹp và xóa tất cả các tệp tin mồ côi (`orphaned_files`) trên ổ đĩa local storage để giải phóng dung lượng đĩa cứng.

---

## Kiểm tra & Bảo trì Storage Audit API

1. **Xem báo cáo audit**:
```bash
curl -H "Authorization: Bearer <YOUR_ACCESS_KEY_OR_JWT>" http://localhost:3000/storage/audit
```

2. **Dọn dẹp file mồ côi trên ổ đĩa (Purge Orphaned Files)**:
```bash
curl -X DELETE -H "Authorization: Bearer <YOUR_ACCESS_KEY_OR_JWT>" http://localhost:3000/storage/audit
```

Kết quả trả về JSON chi tiết số lượng tệp tin đã xóa (`deletedCount`), dung lượng đĩa đã giải phóng (`freedSize`) và danh sách các tệp tin bị xóa.

---

## License

Dự án được phát hành theo giấy phép **MIT License**. Chi tiết xem tại [LICENSE](LICENSE).
