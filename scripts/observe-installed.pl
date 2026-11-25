#!/usr/bin/perl
# Input policy for executing already-built installed witnesses; no Cargo reuse.
use strict;
use warnings;
use File::Find;
use JSON::PP;
use DynaLoader ();
my %files;
sub add { my ($p)=@_; -f $p or die "missing installed input: $p\n"; $files{$p}=1; }
for my $dir ('crates','qualification/ic-wasm-store-qualification','scripts') {
    # File::Find warns and continues on traversal failures. A partial inventory
    # cannot qualify an installed execution for reuse.
    local $SIG{__WARN__} = sub { die @_ };
    find({no_chdir=>1,wanted=>sub {
        -l $_ and die "linked consumer source refuses reuse: $_\n";
        add($_) if -f $_;
    }},$dir);
}
add($_) for qw(Makefile Cargo.toml Cargo.lock qualification/Cargo.toml qualification/Cargo.lock rust-toolchain.toml .toko-tooling.snapshot);
my @executables = ($ENV{STORE_TEST_BINARY},$ENV{STORE_TEST_LAUNCHER},$ENV{STORE_TEST_SERVER},$^X,'/bin/bash');
add('/usr/bin/ldd');
for my $name (qw(IC_WASM_STORE_WASM IC_WASM_STORE_CANDIDATE)) { add($ENV{$name}//die "missing $name\n"); }
for my $exe (@executables) {
    defined($exe) && -x $exe or die "missing executable input\n";
    add($exe);
    open my $in,'-|','/usr/bin/ldd',$exe or die $!;
    local $/; my $text=<$in>; close $in;
    # Statically linked executables have no shared-library closure.
    if ($?) { $text =~ /not a dynamic executable|statically linked/ or die "cannot resolve executable libraries\n"; }
    $text !~ /not found/ or die "missing runtime library\n";
    while ($text =~ m{(/[^\s()]+)}g) { add($1); }
}
add($_) for values %INC, @DynaLoader::dl_shared_objects;
for my $p (qw(/etc/ld.so.cache /etc/ld.so.preload /etc/hosts /etc/resolv.conf /etc/nsswitch.conf /etc/localtime)) { add($p) if -e $p; }
print JSON::PP->new->canonical->utf8->encode({schema=>1,files=>[sort keys %files],configuration=>{contract=>'store-installed-binaries/1'}}),"\n";
