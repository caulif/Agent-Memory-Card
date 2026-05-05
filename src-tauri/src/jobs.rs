use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
static JOB_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopTaskStatus {
    pub job_id: String,
    pub key: String,
    pub stage: String,
    pub lifecycle: String,
    pub label: String,
    pub description: String,
    pub percent: u8,
    pub running: bool,
    pub cancel_requested: bool,
    pub message: String,
    pub details: Vec<DesktopTaskDetail>,
    pub logs: Vec<DesktopTaskLogEntry>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub result_summary: Option<String>,
    #[serde(default)]
    pub replay: Option<DesktopJobReplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopJobReplay {
    pub command: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopTaskDetail {
    pub label: String,
    pub description: String,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopTaskLogEntry {
    pub timestamp: String,
    pub stage: String,
    pub description: String,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopJobStart {
    pub job_id: String,
    pub key: String,
    pub stage: String,
    pub message: String,
    pub accepted: bool,
}

impl Default for DesktopTaskStatus {
    fn default() -> Self {
        Self {
            job_id: String::new(),
            key: String::new(),
            stage: "就绪".to_string(),
            lifecycle: "idle".to_string(),
            label: "就绪".to_string(),
            description: "当前没有正在执行的后台任务".to_string(),
            percent: 100,
            running: false,
            cancel_requested: false,
            message: "就绪".to_string(),
            details: Vec::new(),
            logs: Vec::new(),
            started_at: None,
            finished_at: None,
            result_summary: None,
            replay: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DesktopTaskStore {
    inner: Arc<Mutex<DesktopTaskStatus>>,
    history: Arc<Mutex<Vec<DesktopTaskStatus>>>,
    job_history_path: Option<Arc<PathBuf>>,
}

impl DesktopTaskStore {
    pub fn with_job_history(home: &Path) -> Self {
        let path = home
            .join(".agent-kernel")
            .join("jobs")
            .join("history.jsonl");
        Self {
            inner: Arc::new(Mutex::new(DesktopTaskStatus::default())),
            history: Arc::new(Mutex::new(load_persisted_job_history(&path))),
            job_history_path: Some(Arc::new(path)),
        }
    }

    #[cfg(test)]
    pub fn begin(&self, key: &str, label: &str, description: &str, percent: u8) {
        self.begin_job(key, label, description, percent);
    }

    #[cfg(test)]
    pub fn step(&self, key: &str, label: &str, description: &str, percent: u8) {
        let job_id = self
            .inner
            .lock()
            .ok()
            .filter(|status| status.key == key && !status.job_id.is_empty())
            .map(|status| status.job_id.clone())
            .unwrap_or_else(|| self.begin_job(key, label, description, percent));
        self.job_step(&job_id, label, description, percent);
    }

    #[cfg(test)]
    pub fn finish(&self, key: &str, message: &str) {
        let job_id = self
            .inner
            .lock()
            .ok()
            .filter(|status| status.key == key && !status.job_id.is_empty())
            .map(|status| status.job_id.clone());
        if let Some(job_id) = job_id {
            self.finish_job(&job_id, message);
        } else {
            let job_id = self.begin_job(key, "完成", message, 100);
            self.finish_job(&job_id, message);
        }
    }

    pub fn begin_job(&self, key: &str, stage: &str, description: &str, percent: u8) -> String {
        let job_id = next_job_id(key);
        let now = timestamp_string();
        if let Ok(mut status) = self.inner.lock() {
            let percent = percent.min(100);
            let detail = DesktopTaskDetail {
                label: stage.to_string(),
                description: description.to_string(),
                percent,
            };
            let log = DesktopTaskLogEntry {
                timestamp: now.clone(),
                stage: stage.to_string(),
                description: description.to_string(),
                percent,
            };
            *status = DesktopTaskStatus {
                job_id: job_id.clone(),
                key: key.to_string(),
                stage: stage.to_string(),
                lifecycle: "running".to_string(),
                label: stage.to_string(),
                description: description.to_string(),
                percent,
                running: true,
                cancel_requested: false,
                message: description.to_string(),
                details: vec![detail],
                logs: vec![log],
                started_at: Some(now),
                finished_at: None,
                result_summary: None,
                replay: None,
            };
            self.record_history(status.clone());
        }
        job_id
    }

    pub fn attach_replay(&self, job_id: &str, replay: DesktopJobReplay) {
        let mut updated = None;
        if let Ok(mut status) = self.inner.lock()
            && status.job_id == job_id
        {
            status.replay = Some(replay.clone());
            updated = Some(status.clone());
        }

        if updated.is_none()
            && let Ok(mut jobs) = self.history.lock()
            && let Some(job) = jobs.iter_mut().find(|job| job.job_id == job_id)
        {
            job.replay = Some(replay);
            updated = Some(job.clone());
        }

        if let Some(status) = updated {
            self.record_history(status);
        }
    }

    pub fn job_step(&self, job_id: &str, stage: &str, description: &str, percent: u8) {
        if let Ok(mut status) = self.inner.lock() {
            if status.job_id != job_id {
                return;
            }
            let percent = percent.min(100);
            status.stage = stage.to_string();
            status.lifecycle = "running".to_string();
            status.label = stage.to_string();
            status.description = description.to_string();
            status.percent = percent;
            status.running = true;
            status.message = description.to_string();
            status.finished_at = None;
            status.result_summary = None;
            status.details.push(DesktopTaskDetail {
                label: stage.to_string(),
                description: description.to_string(),
                percent,
            });
            status.logs.push(DesktopTaskLogEntry {
                timestamp: timestamp_string(),
                stage: stage.to_string(),
                description: description.to_string(),
                percent,
            });
            trim_job_history(&mut status);
            self.record_history(status.clone());
        }
    }

    pub fn finish_job(&self, job_id: &str, message: &str) {
        if let Ok(mut status) = self.inner.lock() {
            if status.job_id != job_id {
                return;
            }
            status.stage = "完成".to_string();
            status.lifecycle = if status.cancel_requested {
                "cancelled".to_string()
            } else if message.contains("失败") {
                "failed".to_string()
            } else {
                "completed".to_string()
            };
            status.label = "完成".to_string();
            status.description = message.to_string();
            status.percent = 100;
            status.running = false;
            status.message = message.to_string();
            status.finished_at = Some(timestamp_string());
            status.result_summary = Some(message.to_string());
            status.details.push(DesktopTaskDetail {
                label: "完成".to_string(),
                description: message.to_string(),
                percent: 100,
            });
            status.logs.push(DesktopTaskLogEntry {
                timestamp: timestamp_string(),
                stage: "完成".to_string(),
                description: message.to_string(),
                percent: 100,
            });
            trim_job_history(&mut status);
            self.record_history(status.clone());
        }
    }

    pub fn snapshot(&self) -> DesktopTaskStatus {
        self.inner
            .lock()
            .map(|status| status.clone())
            .unwrap_or_else(|_| DesktopTaskStatus::default())
    }

    pub fn recent_jobs(&self) -> Vec<DesktopTaskStatus> {
        self.history
            .lock()
            .map(|jobs| jobs.clone())
            .unwrap_or_default()
    }

    pub fn request_cancel(&self, job_id: &str) -> Option<DesktopTaskStatus> {
        let mut updated = None;
        if let Ok(mut status) = self.inner.lock()
            && status.job_id == job_id
        {
            status.cancel_requested = true;
            status.lifecycle = "cancelling".to_string();
            status.message = "已请求取消，任务将在下一个安全检查点停止".to_string();
            let description = status.message.clone();
            let percent = status.percent;
            status.logs.push(DesktopTaskLogEntry {
                timestamp: timestamp_string(),
                stage: "请求取消".to_string(),
                description,
                percent,
            });
            trim_job_history(&mut status);
            updated = Some(status.clone());
        }

        if updated.is_none()
            && let Ok(mut jobs) = self.history.lock()
            && let Some(job) = jobs.iter_mut().find(|job| job.job_id == job_id)
        {
            job.cancel_requested = true;
            job.lifecycle = "cancelling".to_string();
            job.message = "已请求取消，任务将在下一个安全检查点停止".to_string();
            updated = Some(job.clone());
        }

        if let Some(status) = &updated {
            self.record_history(status.clone());
        }
        updated
    }

    pub fn cancel_requested(&self, job_id: &str) -> bool {
        self.inner
            .lock()
            .map(|status| status.job_id == job_id && status.cancel_requested)
            .unwrap_or(false)
    }

    fn record_history(&self, status: DesktopTaskStatus) {
        if status.job_id.is_empty() {
            return;
        }
        if let Ok(mut jobs) = self.history.lock() {
            jobs.retain(|job| job.job_id != status.job_id);
            jobs.insert(0, status.clone());
            if jobs.len() > 30 {
                jobs.truncate(30);
            }
        }
        self.persist_history(&status);
    }

    fn persist_history(&self, status: &DesktopTaskStatus) {
        let Some(path) = self.job_history_path.as_deref() else {
            return;
        };
        if let Some(parent) = path.parent()
            && std::fs::create_dir_all(parent).is_err()
        {
            return;
        }
        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        else {
            return;
        };
        if let Ok(line) = serde_json::to_string(status) {
            let _ = writeln!(file, "{line}");
        }
    }

}

fn load_persisted_job_history(path: &Path) -> Vec<DesktopTaskStatus> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut jobs = Vec::new();
    for status in raw
        .lines()
        .filter_map(|line| serde_json::from_str::<DesktopTaskStatus>(line).ok())
        .rev()
    {
        if status.job_id.is_empty()
            || jobs
                .iter()
                .any(|job: &DesktopTaskStatus| job.job_id == status.job_id)
        {
            continue;
        }
        jobs.push(status);
        if jobs.len() >= 30 {
            break;
        }
    }
    jobs
}

fn trim_job_history(status: &mut DesktopTaskStatus) {
    if status.details.len() > 8 {
        status.details = status.details.split_off(status.details.len() - 8);
    }
    if status.logs.len() > 24 {
        status.logs = status.logs.split_off(status.logs.len() - 24);
    }
}

fn sanitize_job_key(key: &str) -> String {
    let sanitized = key
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('-').to_string();
    if sanitized.is_empty() {
        "task".to_string()
    } else {
        sanitized
    }
}

fn next_job_id(key: &str) -> String {
    let seq = JOB_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("job-{}-{}-{seq}", sanitize_job_key(key), unix_millis())
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn timestamp_string() -> String {
    unix_millis().to_string()
}

pub fn job_start_from_status(status: &DesktopTaskStatus, message: &str) -> DesktopJobStart {
    DesktopJobStart {
        job_id: status.job_id.clone(),
        key: status.key.clone(),
        stage: status.stage.clone(),
        message: message.to_string(),
        accepted: !status.job_id.is_empty(),
    }
}
