#!/bin/bash
#/usr/bin/env bash

shopt -s extglob
shopt -s extquote
# shopt -s xpg_echo

set -f

# input waiting latency
declare lat="0.1"
# input of tokens stream as a first parameter
declare input=${1:-""}
# declare inputval=${2:-""}

# declare -a lib_bin_cmds_arr=();
# declare -a lib_bin_names_arr=();
# declare -a layouts_code_arr=();
# declare -a layouts_modules_arr=();

function graphs_layout_templator_gen () {

    declare module="${1}";
    declare lib_bin_cmds="${2}";
    declare lib_bin_names="${3}";

    echo -e "\n"
    echo -e "${module}:\n"
    echo -e "${lib_bin_cmds}\n"
    echo -e "${lib_bin_names}\n"

    readarray -t -n 0 -O 0 -u 0 lib_bin_cmds_arr <<< "${lib_bin_cmds}"
    declare -p lib_bin_cmds_arr;
    readarray -t -n 0 -O 0 -u 0 lib_bin_names_arr <<< "${lib_bin_names}"
    declare -p lib_bin_names_arr;

#    `man dot` documentation:
#    dot - filter for drawing directed graphs
#    neato - filter for drawing undirected graphs
#    twopi - filter for radial layouts of graphs
#    circo - filter for circular layout of graphs
#    fdp - filter for drawing undirected graphs
#    sfdp - filter for drawing large undirected graphs
#    patchwork - filter for squarified tree maps
#    osage - filter for array-based layouts
    layouts_code="dot,twopi,circo,sfdp,fdp,neato,"
    layouts_modules="sfdp,neato,fdp,twopi,circo,dot,"
    echo -e "${layouts_code}\n"
    echo -e "${layouts_modules}\n"

    readarray -d "," -t -n 0 -O 0 -u 0 layouts_code_arr <<< "${layouts_code}";
    unset 'layouts_code_arr[-1]';
    declare -p layouts_code_arr;
    readarray -d "," -t -n 0 -O 0 -u 0 layouts_modules_arr <<< "${layouts_modules}";
    unset 'layouts_modules_arr[-1]';
    declare -p layouts_modules_arr;

    if [[ "${#layouts_code_arr[@]}" -ne ${#layouts_modules_arr[@]} ]]; then
        echo -e "\n"
        echo -e "Layout templates are not equal!\n"
        echo -e "\n"
        return 1
    fi

#    for (( i = 0; i < ${#layouts_code_arr[@]}; i++ )); do
    for i in ${!layouts_code_arr[@]}; do

        if [[ "${#lib_bin_cmds}" -eq 0 && ${#lib_bin_names} -eq 0 ]]; then

            echo -e "\n"
            echo -e "Processing additional layouts for images...\n"
            echo -e "${module}:\n"
            echo -e ":${layouts_code_arr[i]}\n"
            echo -e ":${layouts_modules_arr[i]}\n"

            mkdir -vp "./graphs/${layouts_code_arr[i]}/"
            mkdir -vp "./graphs/${layouts_modules_arr[i]}/"
            cargo modules dependencies --verbose --all-features --no-externs --no-modules --layout "${layouts_code_arr[i]}" --package "${module}" | dot -Tsvg -Eminlen="30" -Gsize="100,56.25" -s100 -Gmindist="100.0" -Glayout="${layouts_code_arr[i]}" "-K${layouts_code_arr[i]}" > "./graphs/${layouts_code_arr[i]}/${module}.types_traits_fns.svg"
            cargo modules dependencies --verbose --all-features --no-fns --no-traits --no-types --layout "${layouts_modules_arr[i]}" --package "${module}" | dot -Tsvg -Eminlen="30" -Gsize="100,56.25" -s100 -Gmindist="100.0" -Glayout="${layouts_modules_arr[i]}" "-K${layouts_modules_arr[i]}" > "./graphs/${layouts_modules_arr[i]}/${module}.crate_modules.svg"

        elif [[ "${#lib_bin_cmds_arr[@]}" -eq ${#lib_bin_names_arr[@]} && "${#lib_bin_cmds_arr[@]}" -ne 0 ]]; then

            echo -e "\n"
            echo -e "Processing additional layouts for images...\n"
            echo -e "${module}:\n"
            echo -e ":${layouts_code_arr[i]}\n"
            echo -e ":${layouts_modules_arr[i]}\n"

            mkdir -vp "./graphs/${layouts_code_arr[i]}/"
            mkdir -vp "./graphs/${layouts_modules_arr[i]}/"
            cargo modules dependencies --verbose --all-features --no-externs --no-modules --layout "${layouts_code_arr[i]}" --package "${module}" ${lib_bin_cmds_arr[i]} | dot -Tsvg -Eminlen="30" -Gsize="100,56.25" -s100 -Gmindist="100.0" -Glayout="${layouts_code_arr[i]}" "-K${layouts_code_arr[i]}" > "./graphs/${layouts_code_arr[i]}/${module}.${lib_bin_names_arr[i]}.types_traits_fns.svg"
            cargo modules dependencies --verbose --all-features --no-fns --no-traits --no-types --layout "${layouts_modules_arr[i]}" --package "${module}" ${lib_bin_cmds_arr[i]} | dot -Tsvg -Eminlen="30" -Gsize="100,56.25" -s100 -Gmindist="100.0" -Glayout="${layouts_modules_arr[i]}" "-K${layouts_modules_arr[i]}" > "./graphs/${layouts_modules_arr[i]}/${module}.${lib_bin_names_arr[i]}.crate_modules.svg"

        fi

    done
}

function modules_graphs_gen () {
    for module in ${2}; do
#        echo -e "${1}:\n"

        echo -e "\n"
        echo -e "${module}:\n"
        echo -e "Processing crate modules...\n"

        output="$(cargo modules dependencies --verbose --all-features --package "${module}" 2>&1)"

        echo -E "${output}" | grep -iP "^-\s.*" | sed -s -r "s/^(-\s)(.*)/\2/gI"
        echo -e "\n"

        lib_bin_cmds="$(echo -E "${output}" | grep -iP "^-\s.*" | sed -s -r "s/^(-\s)(.*)/\2/gI" | grep -iPo "\(.*\)" | sed -s -r "s/(\()(.*)(\))/\2/gI")"
        lib_bin_names="$(echo -E "${output}" | grep -iP "^-\s.*" | sed -s -r "s/^(-\s)(.*)/\2/gI" | grep -iPo "^.*\(" | sed -s -r "s/^(.*)(\s\()/\1/gI")"
        echo -e "${lib_bin_names}:\n"
        echo -e ":${lib_bin_cmds}:\n"

#        read -a lib_bin_cmds_arr -r -d "\n" -e -t ${lat} -u 0 inarray <<< "${lib_bin_cmds}"
#        read -a lib_bin_names_arr -r -d "\n" -e -t ${lat} -u 0 inarray <<< "${lib_bin_names}"

        readarray -t -n 0 -O 0 -u 0 lib_bin_cmds_arr <<< "${lib_bin_cmds}";
        declare -p lib_bin_cmds_arr;
        readarray -t -n 0 -O 0 -u 0 lib_bin_names_arr <<< "${lib_bin_names}";
        declare -p lib_bin_names_arr;

        if [[ "${#lib_bin_cmds}" -eq 0 && ${#lib_bin_names} -eq 0 ]]; then

            echo -e "\n"
            echo -e "Processing images...\n"
            echo -e "${module}:\n"

            cargo modules dependencies --verbose --all-features --no-externs --no-modules --layout twopi --package "${module}" | dot -Tsvg -Eminlen="30" -Gsize="100,56.25" -s100 -Gmindist="100.0" -Glayout="twopi" -Ktwopi > "./graphs/${module}.types_traits_fns.svg"
            cargo modules dependencies --verbose --all-features --no-fns --no-traits --no-types --layout sfdp --package "${module}" | dot -Tsvg -Eminlen="30" -Gsize="100,56.25" -s100 -Gmindist="100.0" -Glayout="sfdp" -Ksfdp > "./graphs/${module}.crate_modules.svg"

            graphs_layout_templator_gen "${module}"

        elif [[ "${#lib_bin_cmds_arr[@]}" -eq ${#lib_bin_names_arr[@]} && "${#lib_bin_cmds_arr[@]}" -ne 0 ]]; then

#            for (( i = 0; i < ${#lib_bin_cmds_arr[@]}; i++ )); do
            for i in ${!lib_bin_cmds_arr[@]}; do

                echo -e "\n"
                echo -e "Processing images...\n"
                echo -e "${module}:\n"
                echo -e "::${lib_bin_names_arr[i]}:\n"
                echo -e ":${lib_bin_cmds_arr[i]}\n"

                cargo modules dependencies --verbose --all-features --no-externs --no-modules --layout twopi --package "${module}" ${lib_bin_cmds_arr[i]} | dot -Tsvg -Eminlen="30" -Gsize="100,56.25" -s100 -Gmindist="100.0" -Glayout="twopi" -Ktwopi > "./graphs/${module}.${lib_bin_names_arr[i]}.types_traits_fns.svg"
                cargo modules dependencies --verbose --all-features --no-fns --no-traits --no-types --layout sfdp --package "${module}" ${lib_bin_cmds_arr[i]} | dot -Tsvg -Eminlen="30" -Gsize="100,56.25" -s100 -Gmindist="100.0" -Glayout="sfdp" -Ksfdp > "./graphs/${module}.${lib_bin_names_arr[i]}.crate_modules.svg"

                graphs_layout_templator_gen "${module}" "${lib_bin_cmds}" "${lib_bin_names}"

            done
        fi

    done
}

mkdir -vp ./graphs/

read -a inputarray -r -d "" -e -t ${lat} -u 0 inarray < /dev/stdin

if [[ "${1}" == "-h" || "${1}" == "--help" ]]; then

    echo -e "usage: graph_modules [-h | --help | input latency ] [input data array]"

elif [[ "${#@}" -eq 0 && "${#inputarray}" -eq 0 ]]; then

    echo -E "$(cargo modules dependencies --verbose --all-features 2>&1 | grep -iP "^-\s.*" | sed -s -r "s/^(-\s)(.*)/\2/gI")" | readarray -d "" -t -n 0 -O 0 -C modules_graphs_gen -c 1 -u 0 inputval < /dev/stdin

elif [[ "${#inputarray}" -ne 0 && ${#input} -ne 0 ]]; then

    echo -E "${inputarray[@]:-"lightning-interfaces"}" | readarray -d "" -t -n 0 -O 0 -C modules_graphs_gen -c 1 -u 0 inputval < /dev/stdin

    echo -E "${input[@]:-"lightning-interfaces"}" | readarray -d "" -t -n 0 -O 0 -C modules_graphs_gen -c 1 -u 0 inputval < /dev/stdin

elif [[ "${#inputarray}" -ne 0 && ${#input} -eq 0 ]]; then

    echo -E "${inputarray[@]:-"lightning-interfaces"}" | readarray -d "" -t -n 0 -O 0 -C modules_graphs_gen -c 1 -u 0 inputval < /dev/stdin

elif [[ "${#inputarray}" -eq 0 && ${#input} -ne 0 ]]; then

    echo -E "${input[@]:-"lightning-interfaces"}" | readarray -d "" -t -n 0 -O 0 -C modules_graphs_gen -c 1 -u 0 inputval < /dev/stdin

else

    echo -E "$(cargo modules dependencies --verbose --all-features 2>&1 | grep -iP "^-\s.*" | sed -s -r "s/^(-\s)(.*)/\2/gI")" | readarray -d "" -t -n 0 -O 0 -C modules_graphs_gen -c 1 -u 0 inputval < /dev/stdin

fi
