# Kang - The crustaceous HTTP server 

![Kang_Logo-removebg-preview](https://github.com/user-attachments/assets/8a93db9c-2a76-4f85-a6f4-5811c023d848)

Kang is a high-performance HTTP server written in Rust, designed for modern web applications. It uses an event-driven architecture with epoll for efficient I/O handling and supports multiple server configurations similar to Nginx.

>[!CAUTION]
>This project is still in very early stages and will undergo continous development. Please do not use for production environments

## Features

- Event-driven architecture using epoll
- Multiple server configurations
- Configurable routes and locations
- CGI support
- Static file serving
- Directory listing
- Custom error pages
- CORS support
- Gzip compression
- Cache control

## Configuration Guide

Kang uses a JSON-based configuration file (`kangrc`). Below is a comprehensive guide to all available options.

### Global Configuration

```json
{
    "global": {
        "worker_processes": 4,                // Required: Number of worker processes
        "worker_connections": 1024,          // Required: Max connections per worker
        "pid": "/var/run/kang.pid",         // Optional: PID file location
        "log_level": "info",                // Optional: Log level (debug, info, warn, error)
        "error_pages": {                     // Optional: Custom error pages
            "root": "/path/to/error/pages",
            "404": "404.html",
            "403": "403.html",
            "500": "500.html"
        },
        "client_max_body_size": "10M"       // Optional: Default max body size
    }
}
```

### Server Configuration

Each server block can have the following options:

```json
{
    "servers": [{
        // Required Options
        "server_name": "example.com",        // Server name(s)
        "host": "0.0.0.0",                  // Binding address
        "ports": [80, 443],                 // Listening ports

        // Optional Options
        "is_default": true,                 // Whether this is the default server
        "backlog": 128,                     // TCP backlog size
        "max_connections": 10000,           // Max concurrent connections
        "client_max_body_size": "100M",     // Override global body size limit
        
        // SSL/TLS Configuration (Optional)
        "ssl": {
            "certificate": "/path/to/cert",
            "certificate_key": "/path/to/key",
            "protocols": ["TLSv1.2", "TLSv1.3"]
        },

        // Timeouts (Optional)
        "timeouts": {
            "read": 60,                     // Read timeout in seconds
            "write": 60,                    // Write timeout in seconds
            "keep_alive": 75                // Keep-alive timeout in seconds
        },

        // TCP Options (Optional)
        "tcp_options": {
            "tcp_nodelay": true,
            "so_keepalive": true,
            "so_reuseaddr": true
        }
    }]
}
```

### Route Configuration

Each server can have multiple routes:

```json
{
    "routes": [{
        // Required Options
        "path": "/",                        // URL path to match
        "root": "/var/www/example",         // Root directory for this route

        // Optional Options
        "methods": ["GET", "POST"],         // Allowed HTTP methods
        "index": ["index.html"],            // Default files for directories
        "directory_listing": false,         // Enable/disable directory listing
        "client_max_body_size": "50M",      // Route-specific body size limit

        // Redirection (Optional)
        "redirect": {
            "url": "/new-path",
            "type": 301                     // 301 or 302
        },

        // CGI Configuration (Optional)
        "cgi": {
            ".php": "/usr/bin/php-cgi",
            ".py": "/usr/bin/python3"
        },

        // CORS Settings (Optional)
        "cors": {
            "enabled": true,
            "allowed_origins": ["*"],
            "allowed_methods": ["GET", "POST"],
            "allowed_headers": ["Content-Type"],
            "max_age": 3600
        },

        // Caching (Optional)
        "cache_control": {
            "max_age": 86400,
            "public": true
        },

        // Compression (Optional)
        "gzip": true,
        "gzip_types": ["text/plain", "text/html"]
    }]
}
```

### Size Units

For size configurations (like `client_max_body_size`), the following units are supported:
- `B` (Bytes)
- `K` or `KB` (Kilobytes)
- `M` or `MB` (Megabytes)
- `G` or `GB` (Gigabytes)

Example: "10M", "1G", "500K"

## Building from Source

```bash
cargo build --release
```

## Running Kang

```bash
kang --config /path/to/kangrc
```

## License

MIT License
