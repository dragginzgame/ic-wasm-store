#!/usr/bin/env perl
# Publish only bounded, sanitized text from explicitly selected repository logs.
use strict;
use warnings;
use Cwd qw(abs_path);
use Fcntl qw(O_RDONLY O_NOFOLLOW);
use File::Temp qw(tempfile);
use FindBin qw($Bin);
use lib "$Bin/../vendor/toko-tooling/scripts/ci";
use SetupDiagnostics qw(excerpt);

my $root = abs_path('.');
my $destination = shift // die "missing evidence directory\n";
my $console_only = $destination eq '--console-only';

sub physical_path {
    my ($path) = @_;
    $path = "$root/$path" unless $path =~ m{^/};
    $path =~ m{^\Q$root\E/} && $path !~ m{(?:^|/)\.\.?(?:/|$)|[\r\n]}
        or die "evidence path must stay inside the repository\n";
    my $part = $path;
    while ($part ne $root) {
        -l $part and die "refusing linked evidence path\n";
        $part =~ s{/[^/]+$}{};
    }
    return $path;
}
unless ($console_only) {
    $destination = physical_path($destination);
    -d $destination or die "missing evidence directory\n";
}
@ARGV or die "no diagnostics selected\n";

my $failed = 0;
for my $source (@ARGV) {
    my $ok = eval {
        $source = physical_path($source);
        -f $source or die "missing diagnostic\n";
        sysopen my $input, $source, O_RDONLY | O_NOFOLLOW or die "cannot open diagnostic: $!\n";
        binmode $input, ':raw' or die "cannot set diagnostic mode: $!\n";
        my $text = excerpt($input, 1024 * 1024);
        close $input or die "cannot close diagnostic: $!\n";
        my $label = $source;
        $label =~ s/([^\x20-\x7e])/sprintf("\\x%02x", ord($1))/ge;
        $text =~ s/^/  /mg;
        $text = "  Source: $label\n$text";
        unless ($console_only) {
            my ($output, $name) = tempfile('diagnostic-XXXXXX', DIR => $destination);
            print {$output} $text or die "cannot write evidence: $!\n";
            close $output or die "cannot close evidence: $!\n";
            print "Sanitized evidence: $name\n";
        }
        my $console = substr($text, 0, 4096);
        print $console, "\n";
        print "  [console excerpt truncated]\n" if length($text) > 4096;
        1;
    };
    unless ($ok) {
        $failed = 1;
        print STDERR "Evidence collection failed: $@";
    }
}
exit $failed;
