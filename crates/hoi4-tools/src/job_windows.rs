//! Windows Job Object that owns a child and its descendants (the Windows counterpart of a Unix
//! process group). Closing the handle or calling `kill` terminates every process in the job.

use std::os::windows::io::RawHandle;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};

pub struct KillJob(HANDLE);

// SAFETY: a job handle is a process-wide kernel handle, usable from any thread.
unsafe impl Send for KillJob {}
unsafe impl Sync for KillJob {}

impl KillJob {
    /// Creates a kill-on-close job and puts the process in it. Returns `None` if Windows
    /// refuses (for example when the process already belongs to a job that forbids nesting).
    pub fn adopt(process: RawHandle) -> Option<KillJob> {
        // SAFETY: plain Win32 calls with valid arguments. The handle is closed on every failure.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return None;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 || AssignProcessToJobObject(job, process as HANDLE) == 0 {
                CloseHandle(job);
                return None;
            }
            Some(KillJob(job))
        }
    }

    /// Terminates every process in the job. Safe to call more than once.
    pub fn kill(&self) {
        // SAFETY: the handle is valid until drop.
        unsafe {
            TerminateJobObject(self.0, 1);
        }
    }
}

impl Drop for KillJob {
    fn drop(&mut self) {
        // SAFETY: closing the handle also kills the job's processes (kill-on-close).
        unsafe {
            CloseHandle(self.0);
        }
    }
}
