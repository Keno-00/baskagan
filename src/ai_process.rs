//! Ensure an owned Codex process tree cannot outlive DADO on Windows.
#[cfg(windows)]
pub struct Job(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
// The handle is exclusively owned and Windows handles can move between threads.
unsafe impl Send for Job {}
#[cfg(windows)]
// Job APIs are thread-safe. Shared references never close the handle; Arc owns
// its lifetime, and only the final owner calls Drop.
unsafe impl Sync for Job {}
#[cfg(windows)]
impl Job {
    /// The primary thread cannot run until tree ownership is established.
    pub fn spawn(
        command: &mut std::process::Command,
    ) -> Result<(std::process::Child, Self), String> {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let result = Self::attach(&child).and_then(|job| {
            Self::resume_primary_thread(&child)?;
            Ok(job)
        });
        match result {
            Ok(job) => Ok((child, job)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(error)
            }
        }
    }
    fn resume_primary_thread(child: &std::process::Child) -> Result<(), String> {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
            System::{
                Diagnostics::ToolHelp::*,
                Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
            },
        };
        // A newly created suspended process has exactly one thread. Enumerate by
        // its owned Child identity, never an executable name or a guessed PID.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut entry: THREADENTRY32 = std::mem::zeroed();
            entry.dwSize = std::mem::size_of_val(&entry) as u32;
            let mut found = Vec::new();
            let mut more = Thread32First(snapshot, &mut entry);
            while more != 0 {
                if entry.th32OwnerProcessID == child.id() {
                    found.push(entry.th32ThreadID);
                }
                more = Thread32Next(snapshot, &mut entry);
            }
            CloseHandle(snapshot);
            if found.len() != 1 {
                return Err("Suspended runtime did not expose exactly one primary thread.".into());
            }
            let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, found[0]);
            if thread.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let resumed = ResumeThread(thread);
            let error = std::io::Error::last_os_error();
            CloseHandle(thread);
            if resumed != 1 {
                return Err(format!(
                    "Could not resume the owned runtime: {error} (suspend count {resumed})"
                ));
            }
            Ok(())
        }
    }
    pub fn terminate(&self) -> Result<(), String> {
        if unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, 1) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    pub fn attach(child: &std::process::Child) -> Result<Self, String> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        // All pointers below refer to live stack data or an owned OS handle.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let job = Self(handle);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(job)
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        os::windows::io::AsRawHandle,
        process::{Command, Stdio},
    };
    struct Cleanup(std::process::Child);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    #[test]
    fn suspended_start_owns_immediate_descendants_before_any_runtime_code_runs() {
        let mut command = Command::new("node");
        command.args(["-e", "const c=require('node:child_process').spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});console.log(c.pid);setInterval(()=>{},1000);"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        let (child, job) = Job::spawn(&mut command).unwrap();
        let mut child = Cleanup(child);
        let mut line = String::new();
        BufReader::new(child.0.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let pid: u32 = line.trim().parse().unwrap();
        unsafe {
            use windows_sys::Win32::{
                Foundation::CloseHandle,
                System::{
                    JobObjects::IsProcessInJob,
                    Threading::{
                        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, WaitForSingleObject,
                    },
                },
            };
            let descendant = OpenProcess(0x00100000 | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            assert!(!descendant.is_null());
            let mut member = 0;
            let queried = IsProcessInJob(descendant, job.0, &mut member);
            job.terminate().unwrap();
            let exited = WaitForSingleObject(descendant, 5000);
            CloseHandle(descendant);
            assert_ne!(queried, 0);
            assert_ne!(member, 0);
            assert_eq!(exited, 0);
            assert_eq!(WaitForSingleObject(child.0.as_raw_handle(), 5000), 0);
        }
    }
    #[test]
    fn termination_stops_owned_descendants_and_leaves_unrelated_process_alive() {
        let unrelated = Cleanup(
            Command::new("node")
                .args(["-e", "setInterval(()=>{},1000)"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let mut child=Cleanup(Command::new("node").args(["-e", "process.stdin.once('data',()=>{const c=require('node:child_process').spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});console.log(c.pid);});setInterval(()=>{},1000);"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap());
        let job = Job::attach(&child.0).unwrap();
        child
            .0
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"start\n")
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.0.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let pid: u32 = line.trim().parse().unwrap();
        unsafe {
            use windows_sys::Win32::{
                Foundation::CloseHandle,
                System::Threading::{OpenProcess, WaitForSingleObject},
            };
            let descendant = OpenProcess(0x00100000, 0, pid); // SYNCHRONIZE only; no mutation by PID.
            assert!(!descendant.is_null());
            job.terminate().unwrap();
            let parent_state = WaitForSingleObject(child.0.as_raw_handle(), 5000);
            let child_state = WaitForSingleObject(descendant, 5000);
            CloseHandle(descendant);
            assert_eq!(parent_state, 0);
            assert_eq!(child_state, 0);
            assert_eq!(WaitForSingleObject(unrelated.0.as_raw_handle(), 0), 258);
        }
    }
}
#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
