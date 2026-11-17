#!/usr/bin/env perl
# Publish only bounded, sanitized text from explicitly selected repository logs.
use strict;
use warnings;
use Cwd qw(abs_path);
use Fcntl qw(O_RDONLY O_NOFOLLOW);
use File::Temp qw(tempfile);

my $root = abs_path('.');
my $destination = shift // die "missing evidence directory\n";
my $limit = 64 * 1024;
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
$destination = physical_path($destination);
-d $destination or die "missing evidence directory\n";
my @secrets = sort { length($b) <=> length($a) }
    map { $ENV{$_} } grep { /TOKEN|SECRET|PASSWORD|CREDENTIAL|PRIVATE_KEY|API_KEY/i
        && defined($ENV{$_}) && length($ENV{$_}) >= 4 } keys %ENV;
sub sanitize {
    my ($text) = @_;
    $text =~ s/\Q$_\E/[REDACTED]/g for @secrets;
    $text =~ s{-----BEGIN [^-\n]*PRIVATE KEY-----.*?(?:-----END [^-\n]*PRIVATE KEY-----|\z)}{[PRIVATE KEY OMITTED]}sg;
    $text =~ s{(https?://)[^/\s\@]+\@}{${1}[REDACTED]\@}gi;
    $text =~ s{(https?://[^\s?]+)\?[^\s]+}{$1?[QUERY OMITTED]}gi;
    $text =~ s{\b(?:gh[pousr]_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|AKIA[A-Z0-9]{16})\b}{[REDACTED]}g;
    $text =~ s{(\b(?:authorization|proxy-authorization)\s*[:=]\s*)[^\r\n]+}{${1}[REDACTED]}gi;
    $text =~ s{(\b[\w.-]*(?:token|secret|password|credential|api[_-]?key)[\w.-]*["']?\s*[:=]\s*)[^\r\n]+}{${1}[REDACTED]}gi;
    $text =~ s/([^\x20-\x7e\n])/sprintf("\\x%02x", ord($1))/ge;
    # Indentation also neutralizes GitHub workflow commands in console excerpts.
    $text =~ s/^/  /mg;
    return $text;
}
my $failed = 0;
for my $source (@ARGV) {
    my $ok = eval {
        $source = physical_path($source);
        -f $source or die "missing diagnostic\n";
        sysopen my $input, $source, O_RDONLY | O_NOFOLLOW or die "cannot open diagnostic: $!\n";
        binmode $input;
        my $count = read $input, my $bytes, 1024 * 1024 + 1;
        defined $count or die "cannot read diagnostic: $!\n";
        close $input or die "cannot close diagnostic: $!\n";
        my $truncated = $count > 1024 * 1024;
        if ($truncated) {
            $bytes = substr($bytes, 0, 1024 * 1024);
            $bytes =~ s/[^\n]*\z//; # Never publish a partially read sensitive field.
        }
        my $text = sanitize("Source: $source\n") . sanitize($bytes);
        $truncated ||= length($text) > $limit;
        if (length($text) > $limit) {
            $text = substr($text, 0, $limit);
            $text =~ s/[^\n]*\z//;
        }
        $text .= "\n  [truncated; remaining raw output omitted]\n" if $truncated;
        $text .= "\n  [empty diagnostic]\n" unless $count;
        my ($output, $name) = tempfile('diagnostic-XXXXXX', DIR => $destination);
        print {$output} $text or die "cannot write evidence: $!\n";
        close $output or die "cannot close evidence: $!\n";
        print "Sanitized evidence: $name\n";
        my $excerpt = substr($text, 0, 4096);
        print $excerpt, "\n";
        print "  [console excerpt truncated; bounded diagnostic retained above]\n" if length($text) > 4096;
        1;
    };
    unless ($ok) {
        $failed = 1;
        print STDERR sanitize("Evidence collection failed: $@"), "\n";
    }
}
@ARGV or die "no diagnostics selected\n";
exit $failed;
