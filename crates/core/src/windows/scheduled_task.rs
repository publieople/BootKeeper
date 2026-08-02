//! Scheduled task enumeration via Task Scheduler COM (windows crate).

use crate::model::{Category, RawEntry};

/// Enumerate scheduled tasks by connecting to the Task Scheduler COM service.
#[cfg(windows)]
pub fn enumerate_scheduled_tasks() -> Vec<RawEntry> {
    use windows::core::BSTR;
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::System::TaskScheduler::{
        CLSID_CTaskScheduler, ITaskFolder, ITaskService,
    };
    use windows::Win32::System::Variant::VARIANT;

    let mut out = Vec::new();

    // CoCreateInstance of the Task Scheduler COM class.
    let service: ITaskService = match unsafe {
        CoCreateInstance(&CLSID_CTaskScheduler, None, CLSCTX_INPROC_SERVER)
    } {
        Ok(s) => s,
        Err(_) => return out,
    };

    // Connect to the local Task Scheduler service.
    let none = VARIANT::default();
    let _ = unsafe { service.Connect(&none, &none, &none, &none) };

    // Root folder.
    let root: ITaskFolder = match unsafe { service.GetFolder(&BSTR::from("\\")) } {
        Ok(f) => f,
        Err(_) => return out,
    };

    // Enumerate tasks in the root folder (non-recursive for v1; recursive is a
    // later improvement — most user tasks live in \ or \Microsoft\...).
    let tasks = match unsafe { root.GetTasks(1) } { // TASK_ENUM_HIDDEN = 1
        Ok(t) => t,
        Err(_) => return out,
    };

    let count = unsafe { tasks.Count() }.unwrap_or(0);
    for i in 1..=count {
        let index = VARIANT::from(i);
        let Ok(task) = (unsafe { tasks.get_Item(&index) }) else {
            continue;
        };
        let name = match unsafe { task.Name() } {
            Ok(n) => n.to_string(),
            Err(_) => continue,
        };
        let path = match unsafe { task.Path() } {
            Ok(p) => p.to_string(),
            Err(_) => String::new(),
        };
        // Resolve the command via the task's first executable action.
        let command = resolve_command(&task);

        out.push(RawEntry::new(Category::ScheduledTask, &path, &name, &command));
    }
    out
}

#[cfg(windows)]
fn resolve_command(task: &windows::Win32::System::TaskScheduler::IRegisteredTask) -> String {
    use windows::Win32::System::TaskScheduler::IExecAction;
    use windows::core::Interface;

    let Ok(def) = (unsafe { task.Definition() }) else {
        return String::new();
    };
    let Ok(actions) = (unsafe { def.Actions() }) else {
        return String::new();
    };
    let mut count = 0i32;
    if (unsafe { actions.Count(&mut count) }).is_err() || count == 0 {
        return String::new();
    }
    let Ok(action) = (unsafe { actions.get_Item(1) }) else {
        return String::new();
    };
    let Ok(exec) = action.cast::<IExecAction>() else {
        return String::new();
    };
    let mut path = windows::core::BSTR::new();
    if (unsafe { exec.Path(&mut path) }).is_err() {
        return String::new();
    }
    path.to_string()
}

#[cfg(not(windows))]
pub fn enumerate_scheduled_tasks() -> Vec<RawEntry> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn compiles() {}
}
