macro_rules! convert {
    ($err:expr) => {
        $err.map_err(|e| -> std::boxed::Box<dyn AnalysisError + '_> { std::boxed::Box::new(e) })
    };
}

macro_rules! log_err {
    ($err:expr) => {
        $err.fail::<()>().unwrap_err().log();
    };
}

/// Unwraps the value, add the value to `vec` and return immediately if error.
macro_rules! unwrap_or {
    ($val:expr => $block:block) => {
        match $val {
            Ok(val) => val,
            Err(e) => {
                e.log();
                $block
            }
        }
    };
    ($val:expr => $stmt:stmt) => {
        match $val {
            Ok(val) => val,
            Err(e) => {
                e.log();
                $stmt
            }
        }
    };
}
