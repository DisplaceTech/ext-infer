--TEST--
Prompt is an immutable, fluent chat-message builder
--SKIPIF--
<?php
if (!extension_loaded('infer')) echo 'skip ext-infer not loaded';
?>
--FILE--
<?php
use Displace\Infer\InferException;
use Displace\Infer\Message;
use Displace\Infer\Prompt;

// Factory + builder.
$p = Prompt::system('You are helpful.')
    ->withUser('What is 2+2?')
    ->withAssistant('4.')
    ->withUser('Now multiply that by 3.');

echo "count: ", $p->count(), "\n";
echo "lastRole: ", $p->lastRole(), "\n";
echo "isEmpty: ", $p->isEmpty() ? "yes" : "no", "\n";

$msgs = $p->messages();
echo "roles: ", implode(',', array_map(fn(Message $m) => $m->role(), $msgs)), "\n";
echo "first_content: ", $msgs[0]->content(), "\n";
echo "second_is_Message: ", ($msgs[1] instanceof Message ? "yes" : "no"), "\n";

// Immutability — every with* returns a new instance.
$base = Prompt::user('hi');
$next = $base->withAssistant('hello');
echo "base_after_with: ", $base->count(), "\n";
echo "next_after_with: ", $next->count(), "\n";

// new Prompt() refused.
try { new Prompt(); echo "FAIL_prompt_ctor\n"; }
catch (InferException $e) { echo "ctor_throws_prompt: yes\n"; }

// new Message() refused.
try { new Message(); echo "FAIL_message_ctor\n"; }
catch (InferException $e) { echo "ctor_throws_message: yes\n"; }

// User-first factory.
$u = Prompt::user('hello');
echo "user_first_role: ", $u->messages()[0]->role(), "\n";
?>
--EXPECT--
count: 4
lastRole: user
isEmpty: no
roles: system,user,assistant,user
first_content: You are helpful.
second_is_Message: yes
base_after_with: 1
next_after_with: 2
ctor_throws_prompt: yes
ctor_throws_message: yes
user_first_role: user
