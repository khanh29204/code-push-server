# ==============================================================================
# Base: Thiết lập môi trường chung
# Sử dụng Bun trên nền Alpine để tối ưu dung lượng (nhỏ gọn ~50MB)
FROM oven/bun:1-alpine AS base
WORKDIR /app
# Cài đặt các công cụ build cần thiết cho module native (như sqlite3)
# Bun vẫn cần python3, make, g++ để biên dịch các gói C++ native
RUN apk add --no-cache python3 make g++ 

# ==============================================================================
# Stage 1: Cài đặt tất cả dependencies (bao gồm devDependencies để build)
FROM base AS deps
COPY package.json bun.lockb* package-lock.json* ./
# Cài đặt toàn bộ dependencies
# Ưu tiên bun.lockb nếu có, fallback sang package-lock.json
RUN bun install --frozen-lockfile || bun install

# ==============================================================================
# Stage 2: Build ứng dụng (TypeScript -> JavaScript)
FROM base AS builder
# Copy node_modules từ bước deps
COPY --from=deps /app/node_modules ./node_modules
COPY . .
# Chạy lệnh build (tsc) để biên dịch ra thư mục bin/
RUN bun run build

# ==============================================================================
# Stage 3: Cài đặt Production Dependencies
# Bước này tách riêng để loại bỏ devDependencies khỏi image cuối cùng -> Giảm RAM & Disk
FROM base AS prod-deps
COPY package.json bun.lockb* package-lock.json* ./
# Chỉ cài production dependencies
RUN bun install --frozen-lockfile --production || bun install --production

# ==============================================================================
# Stage 4: Runner (Image cuối cùng)
# Dùng bản alpine sạch, không cần build tools nữa
FROM oven/bun:1-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production

# Copy production dependencies
COPY --from=prod-deps /app/node_modules ./node_modules

# Copy mã nguồn đã biên dịch (compiled code) và tài nguyên tĩnh
# Lưu ý: Project này build ra thư mục 'bin', views/public/locales không được build nên copy nguyên gốc
COPY --from=builder /app/bin ./bin
COPY --from=builder /app/public ./public
COPY --from=builder /app/views ./views
COPY --from=builder /app/locales ./locales
COPY --from=builder /app/package.json ./

# Tạo symlink để lệnh 'code-push-server-db' trong docker-compose hoạt động
# Thay vì npm install -g, ta link thủ công để tiết kiệm không gian
RUN ln -s /app/bin/db.js /usr/local/bin/code-push-server-db

# Mở cổng
EXPOSE 3000

# Chạy ứng dụng bằng Bun Runtime
# Không dùng PM2 nữa vì Docker sẽ tự restart container nếu crash, và Bun quản lý mem tốt hơn
CMD ["bun", "run", "bin/www.js"]