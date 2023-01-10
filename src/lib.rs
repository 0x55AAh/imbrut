use config::{self, ConfigError, Value, Map};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufRead, Lines};
use http::{Request, request};
use std::{thread, result};
use std::time;
use std::env;
use indicatif::{ProgressBar, ProgressStyle};

enum Result {
    MATCH,
    MISS,
}

trait Proto {
    fn check(&self, password: &str) -> Result;
}

struct PasswordsFile {
    // path: String,
    lines: Lines<BufReader<File>>,
}

impl PasswordsFile {
    fn new(path: &str) -> Self {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        Self { 
            // path: path.to_string(),
            lines: reader.lines(),
        }
    }
}

impl Iterator for PasswordsFile {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.lines.next().and_then(|r| r.ok())
    }
}

struct HTTPProto {
    success_codes: Vec<usize>,
    request: request::Builder,
}

impl HTTPProto {
    fn new(target: &HashMap<String, Value>) -> Self {
        let success_codes = target.get("success_codes").unwrap().clone()
            .into_array()
            .unwrap()
            .into_iter()
            .map(|x| x.into_uint().unwrap() as usize)
            .collect();

        Self { 
            success_codes,
            request: Self::build_request(&target),
        }
    }

    fn build_request(target: &HashMap<String, Value>) -> request::Builder {
        let uri = target.get("uri").unwrap().to_string();
        let method = target.get("method").unwrap().to_string(); // TODO: default POST

        let mut request = Request::builder()
            .method(method.as_str())
            .uri(uri.as_str());

        
        let headers: HashMap<String, String> = target.get("headers").unwrap() // TODO: default empty hashmap
            .clone()
            .into_table()
            .unwrap()
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect();

        for (key, value) in headers {
            request = request.header(key, value);
        }

        request
    }
}

impl Proto for HTTPProto {
    fn check(&self, password: &str) -> Result {
        // let response = send(self.request.body(()).unwrap());

        // TODO: checking
        // println!("Checking: {}", password);

        // Result::MATCH
        Result::MISS
    }
}

struct Settings {
    config: config::Config,
    dict_file: String,
    sleep: u64,
    dict_type: String,
    proto: String,
    target: HashMap<String, Value>,
}

type ConfigResult<T> = result::Result<T, ConfigError>;

impl Settings {
    fn new() -> Self {
        let config_file = env::var("IMBRUT_CONFIG")
            .unwrap_or("config.yml".to_string());
        let dict_file = env::var("IMBRUT_PASSWORDS_FILE")
            .unwrap_or("passwords.txt".to_string());

        let config = config::Config::builder()
            .add_source(config::File::with_name(config_file.as_str()))
            .build()
            .unwrap();  // TODO: create default config?
        
        let sleep = config.get_int("sleep").unwrap_or(0) as u64;
        let dict_type = config.get_string("dict_type")
            .unwrap_or("file".to_string())
            .to_lowercase();
        let proto = config.get_string("proto")
            .unwrap_or("http".to_string())
            .to_lowercase();
        let target = config.get_table("target").unwrap(); // TODO: raise error

        Self { 
            config,
            dict_file,
            sleep,
            dict_type,
            proto,
            target,
        }
    }

    fn save() {
        // TODO: save data into yaml file
    }

    fn get_string(&self, key: &str) -> ConfigResult<String> {
        self.config.get_string(key)
    }

    fn get_int(&self, key: &str) -> ConfigResult<i64> {
        self.config.get_int(key)
    }

    fn get_float(&self, key: &str) -> ConfigResult<f64> {
        self.config.get_float(key)
    }

    fn get_bool(&self, key: &str) -> ConfigResult<bool> {
        self.config.get_bool(key)
    }

    fn get_table(&self, key: &str) -> ConfigResult<Map<String, Value>> {
        self.config.get_table(key)
    }

    fn get_array(&self, key: &str) -> ConfigResult<Vec<Value>> {
        self.config.get_array(key)
    }
}

struct Progress {
    pb: ProgressBar,
}

impl Progress {
    fn new(workload: usize) -> Self {
        let pb = ProgressBar::new(workload as u64);
        Self::customize(&pb);
        Self { pb }
    }

    fn customize(pb: &ProgressBar) {
        let template = "{spinner:.green} [{elapsed_precise}] {percent}% {wide_bar} {human_pos} of {human_len} | {per_sec} | ETA: {eta_precise}";
        pb.set_style(
            ProgressStyle::with_template(template).unwrap()
            // .with_key("eta", |s, w| write!(w, "{}", s.eta().as_secs()).unwrap())
        );
    }

    fn update(&self) {
        self.pb.inc(1);
    }

    fn finish(&self) {
        self.pb.finish();
    }
}


pub struct Application {
    settings: Settings,
    proto: Box<dyn Proto>,
    version: String,
}

impl Application {
    pub fn new() -> Self {
        let settings = Settings::new();
        
        let proto = match settings.proto.as_str() {
            "http" => {
                Box::new(HTTPProto::new(&settings.target))
            }
            _ => {
                // TODO: raise error
                panic!("Unsupported protocol: {}", settings.proto);
            }
        };

        let version = env!("CARGO_PKG_VERSION", "unknown").to_string();

        Self {
            settings,
            proto,
            version,
        }
    }

    fn show_splash(&self) {
        // TODO
        println!("Version: {}", self.version)
    }


    /// Application entrypoint.
    pub fn run(&self) {
        self.show_splash();

        let (passwords, workload) =  match self.settings.dict_type.as_str() {
            "file" => {
                (
                    Box::new(PasswordsFile::new(&self.settings.dict_file)),
                    PasswordsFile::new(&self.settings.dict_file).count(),
                )
            }
            _ => {
                // TODO: raise error
                panic!("Unsupported dict type: {}", self.settings.dict_type);
            }
        };
        let progress = Progress::new(workload);

        for password in passwords {
            match self.proto.check(&password) {
                Result::MATCH => {
                    // TODO
                    break;
                },
                Result::MISS => {
                    // TODO
                },
            }
            if self.settings.sleep > 0 {
                thread::sleep(time::Duration::from_millis(self.settings.sleep));
            }
            progress.update();
        }
        progress.finish();
    }
}