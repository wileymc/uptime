# Uptime Monitor Service Documentation

## Overview
The Uptime Monitor is a Rust-based service that monitors multiple endpoints and sends Slack notifications when their status changes. It runs as a systemd service on a Vultr VPS and provides real-time monitoring with metrics collection.

## Installation Location
- **Code Directory**: `/root/code/uptime`
- **Binary Location**: `/root/code/uptime/target/release/uptime`
- **Service File**: `/etc/systemd/system/uptime.service`
- **Metrics Storage**: `/root/code/uptime/metrics/uptime_metrics.json`

## Current Monitored Endpoints
- `https://api.populist.us`
- `https://api.staging.populist.us`

## Features
- Multi-endpoint monitoring
- Slack notifications for status changes
- Response time tracking
- Uptime percentage calculation
- JSON metrics storage
- Colored console output
- Configurable check intervals and timeouts

## Service Management

### Viewing Service Status
```bash
sudo systemctl status uptime
```

### Starting the Service
```bash
sudo systemctl start uptime
```

### Stopping the Service
```bash
sudo systemctl stop uptime
```

### Restarting the Service
```bash
sudo systemctl restart uptime
```

### Viewing Logs
```bash
# View latest logs
sudo journalctl -u uptime -f

# View logs since last boot
sudo journalctl -u uptime -b

# View last 100 lines
sudo journalctl -u uptime -n 100
```

## Configuration

### Service Configuration File
To modify service settings (like endpoints or intervals), edit the service file:
```bash
sudo nano /etc/systemd/system/uptime.service
```

Current service configuration:
```ini
[Unit]
Description=Uptime Monitor Service
After=network.target

[Service]
Type=simple
Environment=SLACK_WEBHOOK_URL=<your_webhook_url>
ExecStart=/root/code/uptime/target/release/uptime "https://api.populist.us" "https://api.staging.populist.us" --interval 60 --timeout 10
Restart=always
RestartSec=10
WorkingDirectory=/root/code/uptime

[Install]
WantedBy=multi-user.target
```

After any changes to the service file:
```bash
sudo systemctl daemon-reload
sudo systemctl restart uptime
```

## Metrics
Metrics are stored in JSON format at `/root/code/uptime/metrics/uptime_metrics.json`. The file includes:
- Total checks per endpoint
- Successful checks
- Failed checks
- Total downtime
- Average response time
- Last check timestamp
- Last status

To view current metrics:
```bash
cat /root/code/uptime/metrics/uptime_metrics.json
```

## Slack Notifications
The service sends Slack notifications when:
- Service starts up (initial status of endpoints)
- An endpoint changes status (UP → DOWN or DOWN → UP)

Notifications include:
- 🟢 Green circle for UP status
- 🔴 Red circle for DOWN status
- Timestamp
- Response time (for UP status)

## Rebuilding the Service
If code changes are made:
```bash
cd /root/code/uptime
cargo build --release
sudo systemctl restart uptime
```

## Command Line Options
The service accepts these command-line arguments:
- Multiple endpoint URLs (space-separated)
- `--interval` or `-i`: Check interval in seconds (default: 60)
- `--timeout` or `-t`: Request timeout in seconds (default: 10)

Example manual run:
```bash
./target/release/uptime "https://api.populist.us" "https://api.staging.populist.us" --interval 30 --timeout 5
```

## Troubleshooting

### Service Won't Start
1. Check logs:
```bash
sudo journalctl -u uptime -n 50
```

2. Verify binary exists:
```bash
ls -l /root/code/uptime/target/release/uptime
```

3. Check service file permissions:
```bash
ls -l /etc/systemd/system/uptime.service
```

### No Slack Notifications
1. Verify webhook URL in service file
2. Check logs for notification attempts
3. Verify network connectivity to Slack

### High Memory Usage
The service is designed to be lightweight, but if memory issues occur:
1. Check system resources:
```bash
top
free -m
```

2. Restart the service:
```bash
sudo systemctl restart uptime
```

## Maintenance
- Regularly check the metrics file size
- Monitor system logs for any errors
- Keep Rust and dependencies updated
- Consider rotating log files if disk space is a concern

## Security Notes
- The service runs as root (consider creating a dedicated user if needed)
- Webhook URL is stored in plaintext in the service file
- All monitored endpoints must be HTTPS