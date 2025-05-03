#[cfg(target_os = "macos")]
fn main() {
  use std::ops::Div;

  let device = metal::Device::system_default().expect("Get default memory device");

  println!(
    "rec working set size: {} bytes",
    device.recommended_max_working_set_size()
  );
  println!(
    "rec working set size: {} GB",
    device
      .recommended_max_working_set_size()
      .div(1024_u64.pow(3))
  );
}

#[cfg(not(target_os = "macos"))]
fn main() {
  println!("This example does nothing outside macOS")
}
