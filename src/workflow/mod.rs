// Improved code in mod.rs

// Function demonstrating a better variable scope to avoid shadowing
fn example_function() {
    let value = 10;

    // Use a different variable name to avoid shadowing
    let another_value = 20;

    // Improved clarity in variable names and comments
    let result = value + another_value;
    println!("The result is: {}", result);
}