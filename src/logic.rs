use std::fs;
use std::fs::File;
use std::io::Read;
use std::thread;
use std::sync::Mutex;
use lazy_static::lazy_static;

// Define static values
lazy_static! {
    
    static ref CACHE_DIRECTORY: Mutex<String> = Mutex::new(String::new());
    static ref STATUS: Mutex<String> = Mutex::new("Idling".to_owned());
    static ref FILE_LIST: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static ref REQUEST_REPAINT: Mutex<bool> = Mutex::new(false);


    static ref DELETE_TASK_RUNNING: Mutex<bool> = Mutex::new(false);
    static ref LIST_TASK_RUNNING: Mutex<bool> = Mutex::new(false);
    static ref STOP_LIST_RUNNING: Mutex<bool> = Mutex::new(false);
}


const DEFAULT_DIRECTORIES: [&str; 2] = ["%Temp%\\Roblox", "~/.var/app/org.vinegarhq.Sober/cache/sober"];
// For windows and linux (sober)

// Define local functions
fn update_status(value: String) {
    let mut status = STATUS.lock().unwrap();
    *status = value;
    let mut request = REQUEST_REPAINT.lock().unwrap();
    *request = true;
}

fn update_file_list(value: String) {
    let mut file_list = FILE_LIST.lock().unwrap();
    file_list.push(value)
}

fn clear_file_list() {
    let mut file_list = FILE_LIST.lock().unwrap();
    *file_list = Vec::new()
}

// Define public functions
pub fn detect_directory() {
    // Directory detection
    for directory in DEFAULT_DIRECTORIES {
        let resolved_directory = directory
        .replace("%Temp%", &format!("C:\\Users\\{}\\AppData\\Local\\Temp", whoami::username()))
        .replace("~", &format!("/home/{}", whoami::username()));
        // There's probably a better way of doing this... It works though :D

        match fs::metadata(&resolved_directory) { // Directory detection
            Ok(metadata) => {
                if metadata.is_dir() {
                    let mut cache_dir = CACHE_DIRECTORY.lock().unwrap();
                    *cache_dir = resolved_directory;
                    break;
                }
            }
            Err(e) => {
                println!("WARN: {directory}: {e}");
            }
        }

    }
}

pub fn delete_all_directory_contents(dir: String) {
    // Bunch of error checking to check if it's a valid directory
    match fs::metadata(dir.clone()) {
        Ok(metadata) => {
            if metadata.is_dir() {
                let running = {
                    let task = DELETE_TASK_RUNNING.lock().unwrap();
                    task.clone()
                };
                // Stop multiple threads from running
                if running == false {
                    thread::spawn(|| {
                        { 
                            let mut task = DELETE_TASK_RUNNING.lock().unwrap();
                            *task = true;
                        }
                        
                        // Read directory
                        let entries: Vec<_> = fs::read_dir(dir).unwrap().collect();

                        // Get amount and initlilize counter for progress
                        let total = entries.len();
                        let mut count = 0;

                        for entry in entries {
                            count += 1; // Increase counter for progress
                            let path = entry.unwrap().path();
                            if path.is_dir() {
                                match fs::remove_dir_all(path) {
                                    // Error handling and update status
                                    Ok(_) => update_status(format!("Deleting files ({count}/{total})")),

                                    // If it's an error, log it and show on GUI
                                    Err(e) => {
                                        println!("ERROR: Failed to delete file: {}: {}", count, e);
                                        update_status(format!("ERROR: Failed to delete ({count}/{total})"));
                                    }
                                }
                            } else {
                                match fs::remove_file(path) {
                                    // Error handling and update status
                                    Ok(_) => update_status(format!("Deleting files ({count}/{total})")),
    
                                    // If it's an error, log it and show on GUI
                                    Err(e) => {
                                        println!("ERROR: Failed to delete file: {}: {}", count, e);
                                        update_status(format!("ERROR: Failed to delete ({count}/{total})"));
                                    }
                                }    
                            }
                        
                            
                        }
                        { 
                            let mut task = DELETE_TASK_RUNNING.lock().unwrap();
                            *task = false;
                        }
                    });
                }
            // Error handling just so the program doesn't crash for seemingly no reason
            } else {
                let mut status = STATUS.lock().unwrap();
                *status = format!("Error: check logs for more details.");
                println!("ERROR: Directory detection failed.")
            }
        }
        Err(e) => {
            let mut status = STATUS.lock().unwrap();
            *status = format!("Error: '{dir}' is not a valid directory: {e}");
            println!("ERROR: Directory detection failed. {}", format!("'{dir}' is not a valid directory: {e}"))
        }
    }
}

pub fn refresh(dir: String, mode: String) {
    // Bunch of error checking to check if it's a valid directory
    match fs::metadata(dir.clone()) {
        Ok(metadata) => {
            if metadata.is_dir() {
                
                thread::spawn(move || {
                    // This loop here is to make it wait until it is not running, and to set the STOP_LIST_RUNNING to true if it is running to make the other thread
                    loop {
                        let running = {
                            let task = LIST_TASK_RUNNING.lock().unwrap();
                            task.clone()
                        };
                        if !running {
                            break // Break if not running
                        } else {
                            let mut stop = STOP_LIST_RUNNING.lock().unwrap(); // Tell the other thread to stop
                            *stop = true;
                        }
                        thread::sleep(std::time::Duration::from_millis(10)); // Sleep for a bit to not be CPU intensive
                    }
                    { 
                        let mut task = LIST_TASK_RUNNING.lock().unwrap();
                        *task = true;
                        let mut stop = STOP_LIST_RUNNING.lock().unwrap();
                        *stop = false;
                    }

                    clear_file_list(); // Only list the files on the current tab

                    // Read directory
                    let entries: Vec<_> = fs::read_dir(dir).unwrap().collect();

                    // Get amount and initlilize counter for progress
                    let total = entries.len();
                    let mut count = 0;
                    for entry in entries {
                        let stop = {
                            let stop_task = STOP_LIST_RUNNING.lock().unwrap();
                            stop_task.clone()
                        };
                        if stop {
                            break // Stop if another thread requests to stop this task.
                        }
                        
                        count += 1; // Increase counter for progress
                        let path = entry.unwrap().path();
                        let display = path.display();

                        if let Some(filename) = path.file_name() {
                            match &mut File::open(&path) {
                                Err(why) => {
                                    println!("ERROR: couldn't open {}: {}", display, why);
                                    update_status(format!("ERROR: couldn't open ({count}/{total})"));
                                },
                                Ok(file) => {
                                    let mut buffer = vec![0; 2048];
                                    match file.read(&mut buffer) {
                                        Err(why) => {
                                            println!("ERROR: couldn't open {}: {}", display, why);
                                            update_status(format!("ERROR: couldn't open ({count}/{total})"));
                                        },
                                        Ok(bytes_read) => {
                                            buffer.truncate(bytes_read);
                                            
                                            {
                                                update_file_list(filename.to_string_lossy().to_string());   
                                            }
                                            update_status(format!("Reading files ({count}/{total})"));
                                        }
                                    }
                                    
                                },
                            };
                        }
                    }
                    { 
                        let mut task = LIST_TASK_RUNNING.lock().unwrap();
                        *task = false;
                    }
                });
            // Error handling just so the program doesn't crash for seemingly no reason
            } else {
                let mut status = STATUS.lock().unwrap();
                *status = format!("Error: check logs for more details.");
                println!("ERROR: Directory detection failed.")
            }
        }
        Err(e) => {
            let mut status = STATUS.lock().unwrap();
            *status = format!("Error: '{dir}' is not a valid directory: {e}");
            println!("ERROR: Directory detection failed. {}", format!("'{dir}' is not a valid directory: {e}"))
        }
    }
}

pub fn get_file_list() -> Vec<String> {
    FILE_LIST.lock().unwrap().clone()
}

pub fn get_cache_directory() -> String {
    let cache_dir = {
        CACHE_DIRECTORY.lock().unwrap().clone()
    };
    if cache_dir == "" {
        panic!("Panic!ed due to safety. cache_directory was blank! Can possibly DELETE EVERYTHING!")
    } else {
        cache_dir
    }
}

pub fn get_status() -> String {
    STATUS.lock().unwrap().clone()
}


pub fn get_request_repaint() -> bool {
    let mut request_repaint = REQUEST_REPAINT.lock().unwrap();
    let old_request_repaint = request_repaint.clone();
    *request_repaint = false;
    return old_request_repaint
}

pub fn double_click(value: usize) {
    let mut status = STATUS.lock().unwrap();
    *status = format!("Double clicked {}", value);
}