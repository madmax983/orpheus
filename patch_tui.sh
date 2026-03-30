cat << 'DIFF' > replace.patch
--- crates/orpheus-lang/src/tui.rs
+++ crates/orpheus-lang/src/tui.rs
@@ -1259,9 +1259,7 @@
             find_text_in_buffer(&playing_buffer, "Transport: playing")
                 .unwrap_or_else(|| panic!("rendered repl should contain playing status"));
         let playing_x = playing_line_x
-            + u16::try_from("Transport: ".len()).unwrap_or_else(|error| {
-                panic!("transport prefix length should fit in u16: {error}")
-            });
+            + u16::try_from("Transport: ".len()).unwrap();
         let playing_cell = &playing_buffer[(playing_x, playing_y)];
         assert_eq!(playing_cell.fg, Color::Green);
         assert!(playing_cell.modifier.contains(Modifier::BOLD));
@@ -1274,9 +1272,7 @@
             find_text_in_buffer(&stopped_buffer, "Transport: stopped")
                 .unwrap_or_else(|| panic!("rendered repl should contain stopped status"));
         let stopped_x = stopped_line_x
-            + u16::try_from("Transport: ".len()).unwrap_or_else(|error| {
-                panic!("transport prefix length should fit in u16: {error}")
-            });
+            + u16::try_from("Transport: ".len()).unwrap();
         let stopped_cell = &stopped_buffer[(stopped_x, stopped_y)];
         assert_eq!(stopped_cell.fg, Color::Yellow);
         assert!(stopped_cell.modifier.contains(Modifier::BOLD));
@@ -1462,9 +1458,7 @@
         let (repl_line_x, repl_y) = find_text_in_buffer(&repl_buffer, "Transport: syncing")
             .unwrap_or_else(|| panic!("rendered repl should contain syncing status"));
         let repl_x = repl_line_x
-            + u16::try_from("Transport: ".len()).unwrap_or_else(|error| {
-                panic!("transport prefix length should fit in u16: {error}")
-            });
+            + u16::try_from("Transport: ".len()).unwrap();
         let repl_cell = &repl_buffer[(repl_x, repl_y)];
         assert_eq!(repl_cell.fg, Color::Cyan);
         assert!(repl_cell.modifier.contains(Modifier::BOLD));
DIFF
patch -p0 < replace.patch
