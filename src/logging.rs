pub fn init() {
    #[cfg(feature = "logging")]
    env_logger::init();
}

#[macro_export]
macro_rules! log {
    ($level:tt, $arg:expr) => {
        {
            #[cfg(feature="logging")]
            {
                log::$level!($arg);
            }
        }
    };

    ($level:tt, $arg:expr, $($args:expr),+) => {
        {
            #[cfg(feature="logging")]
            {
                log::$level!($arg, $($args),+);
            }
        }
    };
}

pub(crate) use log;
