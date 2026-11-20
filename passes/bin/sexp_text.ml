(* Text <-> [Encore_passes.sexp], matching the Rust reader and printer in
   crates/encore_compiler/src/ir/text/sexp.rs byte for byte, so that IR text
   can go back and forth between the two compilers. See IR.md, "Lexical
   syntax". *)

open Encore_passes

exception Error of int * string
(** A parse error with a 1-based source line. *)

(* Rust's [u8::is_ascii_whitespace]: no vertical tab. *)
let is_space = function ' ' | '\t' | '\n' | '\r' | '\x0c' -> true | _ -> false
let is_digit = function '0' .. '9' -> true | _ -> false

let is_delim c =
  is_space c || match c with '(' | ')' | '"' | ';' | '|' -> true | _ -> false

(* Tokens starting like a number are integers (and must be valid ones). *)
let looks_numeric s =
  let n = String.length s in
  (n >= 1 && is_digit s.[0])
  || (n >= 2 && (s.[0] = '-' || s.[0] = '+') && is_digit s.[1])

(* ── Reader ──────────────────────────────────────────────────────────────── *)

let parse (src : string) : sexp list =
  let len = String.length src in
  let pos = ref 0 and line = ref 1 in
  let err msg = raise (Error (!line, msg)) in
  let peek () = if !pos < len then Some src.[!pos] else None in
  let bump () =
    match peek () with
    | None -> None
    | Some c as r ->
        incr pos;
        if c = '\n' then incr line;
        r
  in
  let rec skip_ws () =
    match peek () with
    | Some c when is_space c -> ignore (bump ()); skip_ws ()
    | Some ';' ->
        while (match peek () with Some '\n' | None -> false | Some _ -> true) do
          ignore (bump ())
        done;
        skip_ws ()
    | _ -> ()
  in
  let escaped close =
    let buf = Buffer.create 16 in
    let hex () =
      match bump () with
      | Some ('0' .. '9' as c) -> Char.code c - 48
      | Some ('a' .. 'f' as c) -> Char.code c - 87
      | Some ('A' .. 'F' as c) -> Char.code c - 55
      | _ -> err "expected two hex digits after `\\x`"
    in
    let rec go () =
      match bump () with
      | None -> err (Printf.sprintf "unterminated `%c`" close)
      | Some c when c = close -> Buffer.contents buf
      | Some '\\' ->
          (match bump () with
           | Some 'n' -> Buffer.add_char buf '\n'
           | Some 't' -> Buffer.add_char buf '\t'
           | Some 'r' -> Buffer.add_char buf '\r'
           | Some 'x' ->
               let hi = hex () in
               let lo = hex () in
               Buffer.add_char buf (Char.chr ((hi lsl 4) lor lo))
           | Some (('\\' | '"' | '|') as c) -> Buffer.add_char buf c
           | Some c -> err (Printf.sprintf "unknown escape `\\%c`" c)
           | None -> err "unterminated escape");
          go ()
      | Some c -> Buffer.add_char buf c; go ()
    in
    go ()
  in
  (* Rust's [str::parse::<i64>]: an optional sign, then decimal digits. *)
  let integer tok =
    let digits =
      if tok.[0] = '-' || tok.[0] = '+' then String.sub tok 1 (String.length tok - 1)
      else tok
    in
    let invalid () = err (Printf.sprintf "invalid integer `%s`" tok) in
    if not (String.for_all is_digit digits) then invalid ();
    let tok = if tok.[0] = '+' then digits else tok in
    match int_of_string_opt tok with Some i -> i | None -> invalid ()
  in
  let rec node () =
    let start_line = !line in
    match peek () with
    | Some '(' ->
        ignore (bump ());
        let rec items acc =
          skip_ws ();
          match peek () with
          | None -> raise (Error (start_line, "unclosed `(`"))
          | Some ')' -> ignore (bump ()); List.rev acc
          | Some _ -> items (node () :: acc)
        in
        List (items [])
    | Some ')' -> err "unexpected `)`"
    | Some '"' -> ignore (bump ()); Str (escaped '"')
    | Some '|' ->
        ignore (bump ());
        let s = escaped '|' in
        if String.is_valid_utf_8 s then Sym s else err "symbol is not valid UTF-8"
    | Some _ ->
        let start = !pos in
        while (match peek () with Some c -> not (is_delim c) | None -> false) do
          ignore (bump ())
        done;
        let tok = String.sub src start (!pos - start) in
        if looks_numeric tok then Int (integer tok) else Sym tok
    | None -> err "unexpected end of input"
  in
  let rec top acc =
    skip_ws ();
    if !pos >= len then List.rev acc else top (node () :: acc)
  in
  top []

(* ── Printer ─────────────────────────────────────────────────────────────── *)

let width = 80

(* Lists up to this width may stay on their parent's opening line. *)
let inline_arg_width = 32

let is_graphic c = c >= '\x21' && c <= '\x7e'

let write_sym buf s =
  let plain =
    s <> "" && (not (looks_numeric s))
    && String.for_all (fun c -> (not (is_delim c)) && is_graphic c) s
  in
  if plain then Buffer.add_string buf s
  else begin
    Buffer.add_char buf '|';
    String.iter
      (function
        | '|' -> Buffer.add_string buf "\\|"
        | '\\' -> Buffer.add_string buf "\\\\"
        | '\n' -> Buffer.add_string buf "\\n"
        | '\t' -> Buffer.add_string buf "\\t"
        | '\r' -> Buffer.add_string buf "\\r"
        | c -> Buffer.add_char buf c)
      s;
    Buffer.add_char buf '|'
  end

let write_str buf s =
  Buffer.add_char buf '"';
  String.iter
    (function
      | '"' -> Buffer.add_string buf "\\\""
      | '\\' -> Buffer.add_string buf "\\\\"
      | '\n' -> Buffer.add_string buf "\\n"
      | '\t' -> Buffer.add_string buf "\\t"
      | '\r' -> Buffer.add_string buf "\\r"
      | '\x20' .. '\x7e' as c -> Buffer.add_char buf c
      | c -> Buffer.add_string buf (Printf.sprintf "\\x%02x" (Char.code c)))
    s;
  Buffer.add_char buf '"'

let rec write_flat buf = function
  | Sym s -> write_sym buf s
  | Int i -> Buffer.add_string buf (string_of_int i)
  | Str s -> write_str buf s
  | List items ->
      Buffer.add_char buf '(';
      List.iteri
        (fun i item ->
          if i > 0 then Buffer.add_char buf ' ';
          write_flat buf item)
        items;
      Buffer.add_char buf ')'

let atom_width e =
  let buf = Buffer.create 16 in
  write_flat buf e;
  Buffer.length buf

(* Flat width of [e], or [None] once it exceeds [budget] (keeps layout linear). *)
let rec flat_width e budget =
  let w =
    match e with
    | Sym _ | Int _ | Str _ -> Some (atom_width e)
    | List items ->
        List.fold_left
          (fun acc item ->
            match acc with
            | None -> None
            | Some w ->
                if w > budget then None
                else Option.map (( + ) w) (flat_width item (budget - w)))
          (Some (2 + max 0 (List.length items - 1)))
          items
  in
  match w with Some w when w <= budget -> Some w | _ -> None

(* A list too wide for one line keeps its head and leading atoms / short lists
   on the opening line (never the last item); the remaining items are broken
   onto indented lines. *)
let rec write_pretty buf e indent =
  match e with
  | List (head :: _ as items) when flat_width e (max 0 (width - indent)) = None ->
      let items = Array.of_list items in
      let n = Array.length items in
      let split = ref 1 in
      let col = ref (indent + 1 + Option.get (flat_width head max_int)) in
      (try
         while !split + 1 < n do
           let budget =
             match items.(!split) with List _ -> inline_arg_width | _ -> max_int
           in
           match flat_width items.(!split) budget with
           | Some w when !col + 1 + w <= width ->
               col := !col + 1 + w;
               incr split
           | _ -> raise Exit
         done
       with Exit -> ());
      Buffer.add_char buf '(';
      write_pretty buf head (indent + 1);
      for i = 1 to !split - 1 do
        Buffer.add_char buf ' ';
        write_flat buf items.(i)
      done;
      for i = !split to n - 1 do
        Buffer.add_char buf '\n';
        Buffer.add_string buf (String.make (indent + 2) ' ');
        write_pretty buf items.(i) (indent + 2)
      done;
      Buffer.add_char buf ')'
  | _ -> write_flat buf e

(* Top-level forms separated by blank lines, after a header comment. *)
let render_module header forms =
  let buf = Buffer.create 4096 in
  Buffer.add_string buf ("; " ^ header ^ "\n");
  List.iter
    (fun f ->
      Buffer.add_char buf '\n';
      write_pretty buf f 0;
      Buffer.add_char buf '\n')
    forms;
  Buffer.contents buf
