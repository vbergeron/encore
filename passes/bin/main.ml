(* encore-passes: run the Rocq passes on IR text.

   Reads IR text that is the output of stage --from, runs every pass after it
   up to and including --emit (default: --from itself, i.e. a round trip),
   and prints the resulting IR text. Same stage names and meaning as
   `encore compile ir --from/--emit` (see IR.md). *)

open Encore_passes

let usage =
  Printf.sprintf
    "Usage: encore-passes [INPUT] --from STAGE [--emit STAGE] [-o OUTPUT]\n\n\
     Reads IR text produced by STAGE (from INPUT, or standard input when INPUT\n\
     is omitted or `-`) and runs the Rocq passes after it up to --emit.\n\n\
     Stages: %s\n\n\
     Options:"
    (String.concat ", " stage_names)

let die fmt = Printf.ksprintf (fun msg -> prerr_endline ("encore-passes: " ^ msg); exit 1) fmt

let read_all ic = In_channel.input_all ic

let stage_arg flag = function
  | None -> None
  | Some name ->
      (match stage_of_name name with
       | Some s -> Some s
       | None ->
           die "%s: unknown stage `%s` (expected one of: %s)" flag name
             (String.concat ", " stage_names))

let () =
  let input = ref None and output = ref None in
  let from = ref None and emit = ref None in
  let set r v = r := Some v in
  let specs =
    Arg.align
      [ ("--from", Arg.String (set from), "STAGE The stage that produced INPUT (required)");
        ("--emit", Arg.String (set emit), "STAGE Stop after this stage (default: --from)");
        ("-o", Arg.String (set output), "OUTPUT Write to OUTPUT instead of standard output") ]
  in
  let anon a =
    match !input with
    | None -> input := Some a
    | Some _ -> raise (Arg.Bad ("unexpected argument " ^ a))
  in
  Arg.parse specs anon usage;
  let from =
    match stage_arg "--from" !from with
    | Some s -> s
    | None -> prerr_string (Arg.usage_string specs usage); exit 2
  in
  let emit = Option.value (stage_arg "--emit" !emit) ~default:from in
  let name, src =
    match !input with
    | None | Some "-" -> ("<stdin>", read_all stdin)
    | Some path ->
        (try (path, In_channel.with_open_bin path read_all)
         with Sys_error e -> die "%s" e)
  in
  let forms =
    try Sexp_text.parse src
    with Sexp_text.Error (line, msg) -> die "%s: line %d: %s" name line msg
  in
  match compile from emit forms with
  | Err msg -> die "%s: %s" name msg
  | Ok (header, forms) ->
      let text = Sexp_text.render_module header forms in
      (match !output with
       | None -> print_string text
       | Some path ->
           (try Out_channel.with_open_bin path (fun oc -> Out_channel.output_string oc text)
            with Sys_error e -> die "%s" e))
