#!/usr/bin/env bash
# Render bounded retained diagnostics; leave original evidence untouched.
set -euo pipefail
if (( $# == 0 )); then
    echo 'No simulator setup evidence was produced; inspect the setup failure above.'
fi
for file in "$@"; do
    if [[ ! -f "$file" || -L "$file" || -L "${file%/*}" ]]; then
        printf 'No simulator setup evidence available at %q\n' "$file"
        continue
    fi
    perl -e '
        use strict; use warnings;
        sub escaped { my ($text) = @_; $text =~ s/([^\x20-\x7e\n])/sprintf("\\x%02x", ord($1))/ge; return $text; }
        my $file = shift;
        open my $input, "<:raw", $file or die "cannot read diagnostic: $!\n";
        my $count = read $input, my $bytes, 4097;
        defined $count or die "cannot read diagnostic: $!\n";
        my $label = escaped($file); $label =~ s/\n/\\n/g;
        print "\n--- $label (first 4096 bytes) ---\n";
        my $text = escaped(substr($bytes, 0, 4096));
        $text =~ s/^/  /mg;
        print $text, "\n";
        print "  [truncated; full diagnostic retained in the file above]\n" if $count > 4096;
    ' "$file"
done
