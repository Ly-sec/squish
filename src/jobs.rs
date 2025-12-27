use std::process::Child;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Job {
    pub id: usize,
    pub command: String,
    pub child: Arc<Mutex<Option<Child>>>,
}

pub struct JobManager {
    jobs: Vec<Job>,
    next_id: usize,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: Vec::new(),
            next_id: 1,
        }
    }

    pub fn add_job(&mut self, command: String, child: Child) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let job = Job {
            id,
            command,
            child: Arc::new(Mutex::new(Some(child))),
        };
        self.jobs.push(job);
        id
    }

    pub fn list_jobs(&self) -> &[Job] {
        &self.jobs
    }

    pub fn get_job(&mut self, id: usize) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    pub fn remove_finished(&mut self) {
        self.jobs.retain(|job| {
            if let Ok(mut child_opt) = job.child.lock() {
                if let Some(ref mut child) = *child_opt {
                    if let Ok(Some(_)) = child.try_wait() {
                        *child_opt = None;
                        return false;
                    }
                } else {
                    return false;
                }
            }
            true
        });
    }
}





