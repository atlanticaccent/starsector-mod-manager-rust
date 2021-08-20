use tinyfiledialogs as tfd;

pub fn error<T: AsRef<str>>(message: T) {
  tfd::message_box_ok("Error", message.as_ref(), tfd::MessageBoxIcon::Error);
}

pub fn notif<T: AsRef<str>>(message: T) {
  tfd::message_box_ok("Message:", message.as_ref(), tfd::MessageBoxIcon::Info);
}

pub fn query<T: AsRef<str>>(message: T) -> bool {
  match tfd::message_box_yes_no("Query:", message.as_ref(), tfd::MessageBoxIcon::Question, tfd::YesNo::No) {
    tfd::YesNo::Yes => true,
    tfd::YesNo::No => false
  }
}
