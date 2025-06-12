/* src/main.rs */

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuItem, NSStatusBar,
    NSVariableStatusItemLength,
};
use cocoa::base::{id, nil, YES};
use cocoa::foundation::{NSString, NSAutoreleasePool};
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use sysinfo::{System};

struct AppState {
    status_item: id,
    metrics: Arc<Mutex<SystemMetrics>>,
}

#[derive(Clone, Default)]
struct SystemMetrics {
    cpu_usage: String,
    ram_usage: String,
}

extern "C" fn update_title(this: &Object, _cmd: Sel, _timer: id) {
    unsafe {
        let app_state_ptr: *mut c_void = *this.get_ivar("appState");
        if app_state_ptr.is_null() {
            return;
        }
        let app_state = &*(app_state_ptr as *mut AppState);
        let data = app_state.metrics.lock().unwrap().clone();
        let display_text = format!("{} {}", data.cpu_usage, data.ram_usage);
        let ns_str = make_nsstring(&display_text);
        let _: () = msg_send![app_state.status_item, setTitle: ns_str];
    }
}

fn make_nsstring(s: &str) -> id {
    unsafe { NSString::alloc(nil).init_str(s) }
}

fn update_cpu_loop(metrics: Arc<Mutex<SystemMetrics>>) {
    thread::spawn(move || {
        let mut sys = System::new();
        loop {
            sys.refresh_cpu();
            let usage = sys.global_cpu_info().cpu_usage();
            let mut data = metrics.lock().unwrap();
            data.cpu_usage = format!("{:.0}%", usage);
            drop(data);
            thread::sleep(Duration::from_millis(1200));
        }
    });
}

// better accuracy on macOS
fn update_ram_loop(metrics: Arc<Mutex<SystemMetrics>>) {
    thread::spawn(move || {
        let mut sys = System::new_all();

        loop {
            sys.refresh_memory();

            let used_memory_b = sys.total_memory() - sys.available_memory();
            let used_gb = used_memory_b as f64 / (1024.0 * 1024.0 * 1024.0);

            let mem_str = format!("{:.1}GB", used_gb);

            let mut data = metrics.lock().unwrap();
            data.ram_usage = mem_str;
            drop(data);

            thread::sleep(Duration::from_millis(2100));
        }
    });
}

fn create_quit_menu() -> id {
    unsafe {
        let menu = NSMenu::new(nil).autorelease();
        let quit_title = make_nsstring("Quit");
        let q_key = make_nsstring("q");
        let quit_item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            quit_title,
            sel!(terminate:),
            q_key,
        );
        menu.addItem_(quit_item);
        menu
    }
}

fn main() {
    let shared_metrics = Arc::new(Mutex::new(SystemMetrics::default()));
    update_cpu_loop(Arc::clone(&shared_metrics));
    update_ram_loop(Arc::clone(&shared_metrics));

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory);

        let status_item = NSStatusBar::systemStatusBar(nil).statusItemWithLength_(NSVariableStatusItemLength);

        let mut app_state = Box::new(AppState {
            status_item,
            metrics: Arc::clone(&shared_metrics),
        });

        {
            let data = app_state.metrics.lock().unwrap();
            let initial_text = format!("🖥 {} 🧠 {}", data.cpu_usage, data.ram_usage);
            let _: () = msg_send![status_item, setTitle: make_nsstring(&initial_text)];
        }

        let superclass = class!(NSObject);
        let mut decl = ClassDecl::new("PulseTimerTarget", superclass).unwrap();
        decl.add_ivar::<*mut c_void>("appState");
        decl.add_method(sel!(updateTitle:), update_title as extern "C" fn(&Object, Sel, id));
        let timer_target_class = decl.register();
        let timer_target: id = msg_send![timer_target_class, new];

        (*timer_target).set_ivar("appState", &mut *app_state as *mut _ as *mut c_void);

        let timer: id = msg_send![class!(NSTimer), timerWithTimeInterval:0.5
            target:timer_target
            selector:sel!(updateTitle:)
            userInfo:nil
            repeats:YES
        ];

        let run_loop: id = msg_send![class!(NSRunLoop), mainRunLoop];
        let common_modes = make_nsstring("kCFRunLoopCommonModes");
        let _: () = msg_send![run_loop, addTimer: timer forMode: common_modes];

        let menu = create_quit_menu();
        let _: () = msg_send![status_item, setMenu: menu];

        app.run();
    }
}
