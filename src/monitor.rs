use chrono::{DateTime, Utc};
use colored::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
use tokio::time::sleep;
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct Metrics {
    endpoint: String,
    total_checks: u64,
    successful_checks: u64,
    failed_checks: u64,
    total_downtime: u64,
    last_check: Option<DateTime<Utc>>,
    last_status: Option<String>,
    average_response_time: f64,
}

impl Metrics {
    fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            total_checks: 0,
            successful_checks: 0,
            failed_checks: 0,
            total_downtime: 0,
            last_check: None,
            last_status: None,
            average_response_time: 0.0,
        }
    }
}

pub struct Monitor {
    endpoints: Vec<String>,
    check_interval: Duration,
    timeout: Duration,
    metrics: HashMap<String, Metrics>,
    client: Client,
    slack_webhook_url: Option<String>,
}

impl Monitor {
    pub fn new(endpoints: Vec<String>, check_interval: Duration, timeout: Duration) -> Self {
        let slack_webhook_url = std::env::var("SLACK_WEBHOOK_URL").ok();

        let client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        let metrics = endpoints
            .iter()
            .map(|endpoint| (endpoint.clone(), Metrics::new(endpoint.clone())))
            .collect();

        Self {
            endpoints,
            check_interval,
            timeout,
            metrics,
            client,
            slack_webhook_url,
        }
    }

    async fn check_endpoint(&self, endpoint: &str) -> (bool, f64) {
        let start = Instant::now();

        match self.client.get(endpoint).send().await {
            Ok(response) => {
                let duration = start.elapsed().as_secs_f64();
                let success = response.status().is_success();
                (success, duration)
            }
            Err(e) => {
                error!("Request failed for {}: {}", endpoint, e);
                (false, 0.0)
            }
        }
    }

    async fn send_slack_notification(
        &self,
        endpoint: &str,
        is_down: bool,
        response_time: Option<f64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "=== Starting Slack notification process for {} ===",
            endpoint
        );

        let webhook_url = match &self.slack_webhook_url {
            Some(url) => {
                info!("Found webhook URL: [webhook url]");
                url
            }
            None => {
                error!("No webhook URL configured!");
                return Ok(());
            }
        };

        let message = if is_down {
            format!(
                "🔴 {} is DOWN! (Time: {})",
                endpoint,
                Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
            )
        } else {
            format!(
                "🟢 {} is back UP! (Time: {}, Response Time: {:.2}s)",
                endpoint,
                Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
                response_time.unwrap_or(0.0)
            )
        };

        info!("Preparing to send message: {}", message);

        let payload = serde_json::json!({
            "text": message
        });

        info!("Sending request to Slack...");

        match self
            .client
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(res) => {
                let status = res.status();
                match res.text().await {
                    Ok(text) => {
                        info!("Slack response - Status: {}, Body: {}", status, text);
                        if !status.is_success() {
                            error!("Failed to send Slack notification! Status: {}", status);
                        } else {
                            info!("Slack notification sent successfully!");
                        }
                    }
                    Err(e) => error!("Failed to read Slack response: {}", e),
                }
            }
            Err(e) => error!("Failed to send request to Slack: {}", e),
        };

        info!("=== Finished Slack notification process ===");
        Ok(())
    }

    fn update_metrics(&mut self, endpoint: &str, success: bool, response_time: f64) {
        let metrics = self.metrics.get_mut(endpoint).unwrap();

        metrics.total_checks += 1;
        metrics.last_check = Some(Utc::now());
        metrics.last_status = Some(if success { "up".into() } else { "down".into() });

        if success {
            metrics.successful_checks += 1;
            let prev_avg = metrics.average_response_time;
            metrics.average_response_time = (prev_avg * (metrics.successful_checks as f64 - 1.0)
                + response_time)
                / metrics.successful_checks as f64;
        } else {
            metrics.failed_checks += 1;
            metrics.total_downtime += self.check_interval.as_secs();
        }

        // Save metrics to file
        if let Err(e) = self.save_metrics() {
            error!("Failed to save metrics: {}", e);
        }
    }

    fn save_metrics(&self) -> std::io::Result<()> {
        fs::create_dir_all("metrics")?;
        let metrics_path = Path::new("metrics/uptime_metrics.json");
        let mut file = File::create(metrics_path)?;
        let json = serde_json::to_string_pretty(&self.metrics)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub async fn run(&mut self) {
        info!(
            "Starting uptime monitoring for {} endpoints",
            self.endpoints.len()
        );

        // Verify webhook configuration
        match &self.slack_webhook_url {
            Some(_) => info!("Slack webhook configured"),
            None => error!("No Slack webhook URL configured - notifications will not be sent"),
        };

        // Initial check for all endpoints
        let endpoints: Vec<String> = self.endpoints.clone();
        for endpoint in &endpoints {
            info!("Performing initial status check for {}", endpoint);
            let (success, response_time) = self.check_endpoint(endpoint).await;
            info!(
                "Initial check result for {} - Success: {}",
                endpoint, success
            );

            // Force initial notification
            info!("Forcing initial notification for {}", endpoint);
            if let Err(e) = self
                .send_slack_notification(endpoint, !success, Some(response_time))
                .await
            {
                error!(
                    "Failed to send initial notification for {}: {:?}",
                    endpoint, e
                );
            }

            self.update_metrics(endpoint, success, response_time);
        }

        // Start monitoring loop
        loop {
            sleep(self.check_interval).await;

            let endpoints: Vec<String> = self.endpoints.clone();
            for endpoint in &endpoints {
                let (success, response_time) = self.check_endpoint(endpoint).await;

                if let Some(metrics) = self.metrics.get(endpoint) {
                    if let Some(last_status) = &metrics.last_status {
                        let status_changed =
                            (last_status == "up" && !success) || (last_status == "down" && success);
                        info!(
                            "Status check for {} - Last: {}, Current: {}, Changed: {}",
                            endpoint,
                            last_status,
                            if success { "up" } else { "down" },
                            status_changed
                        );

                        if status_changed {
                            info!("Status changed for {} - sending notification", endpoint);
                            if let Err(e) = self
                                .send_slack_notification(endpoint, !success, Some(response_time))
                                .await
                            {
                                error!("Failed to send notification for {}: {:?}", endpoint, e);
                            }
                        }
                    }
                }

                self.update_metrics(endpoint, success, response_time);

                let (status_emoji, status_color) = if success {
                    ("🟢", "UP".green().bold())
                } else {
                    ("🔴", "DOWN".red().bold())
                };

                let metrics = self.metrics.get(endpoint).unwrap();
                info!(
                    "{} {} {} | ⏱️  {:.2}s | 📈 {:.2}%",
                    status_emoji,
                    endpoint,
                    status_color,
                    response_time,
                    (metrics.successful_checks as f64 / metrics.total_checks as f64) * 100.0
                );
            }
        }
    }
}
