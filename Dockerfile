# ==============================================================================
# Giai đoạn 1: Cài đặt dependencies (deps stage)
# Sử dụng Alpine Node LTS base image
FROM node:lts-alpine AS deps

# Tạo thư mục làm việc
WORKDIR /app

# Chỉ copy các file khai báo dependencies (package.json và lock file)
COPY package.json package-lock.json* ./

# Cài đặt TẤT CẢ dependencies (bao gồm devDependencies)
RUN npm ci

# ==============================================================================
# Giai đoạn 2: Build ứng dụng (builder stage)
# Sử dụng Alpine Node LTS base image
FROM node:lts-alpine AS builder

# Tạo thư mục làm việc
WORKDIR /app

# Sao chép node_modules đã cài đặt từ giai đoạn deps
COPY --from=deps /app/node_modules ./node_modules

# Sao chép toàn bộ mã nguồn từ build context
# Đảm bảo sử dụng file .dockerignore để loại trừ các thư mục không cần thiết
COPY . .

# Chạy lệnh build của ứng dụng (nếu có trong package.json)
RUN npm run build

# Loại bỏ dev dependencies sau khi build xong
RUN npm prune --production

# ==============================================================================
# Giai đoạn 3: Chạy ứng dụng (runner stage)
# Sử dụng Alpine Node LTS base image - nhỏ gọn và chứa sẵn Node
FROM node:lts-alpine AS runner

# Cài đặt PM2 global để dùng pm2-runtime
RUN npm install -g pm2@latest

# Tạo thư mục làm việc cho ứng dụng trong image cuối cùng
WORKDIR /app

# Sao chép các file cần thiết từ giai đoạn builder:
# 1. production node_modules (đã prune ở builder)
# 2. Mã nguồn ứng dụng và kết quả build (giả định nằm trong /app sau bước build)
# 3. package.json
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app .

# Cài đặt chính project này như một global package, theo yêu cầu của bạn
# Bước này chạy trong giai đoạn runner sau khi đã có code và node_modules
RUN npm install -g .

# Sao chép file cấu hình process.json từ build context (hoặc nó đã được copy ở bước trên)
# Giả định process.json nằm ở thư mục gốc của project
# Nếu bạn đã dùng 'COPY --from=builder /app .' và process.json nằm ở gốc project,
# nó đã được copy rồi. Bỏ comment dòng dưới nếu process.json ở vị trí khác và cần copy riêng.
# COPY ./process.json /app/process.json

# Mở cổng mặc định mà CodePush server sử dụng (thường là 3000)
EXPOSE 3000

# Lệnh mặc định để khởi chạy ứng dụng bằng pm2-runtime
CMD ["pm2-runtime", "/app/process.json"]