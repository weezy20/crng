//! Helper functions for formatting and utilities

/// Formats a number with comma separators for better readability
/// Example: 1234567 -> "1,234,567"
pub fn format_number_with_commas(num: u64) -> String {
    let num_str = num.to_string();
    let chars: Vec<char> = num_str.chars().collect();
    let mut result = String::new();
    
    for (i, &ch) in chars.iter().enumerate() {
        // Add comma every 3 digits from the right
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    
    result
}