<?php

/*
 * examples/chat-interactive/src/ChatCommand.php
 *
 * A Symfony Console command that runs a multi-turn chat against a local
 * GGUF model via ext-infer. Demonstrates:
 *
 *   - building up a `Prompt` across multiple user/assistant turns
 *   - keeping the system message stable across `/reset`
 *   - reading `Response::reasoning()` separately from `Response::answer()`
 *     when the model is a reasoning model (Qwen3, R1, …)
 *   - graceful handling of `InferException` mid-conversation
 *
 * The file is intentionally chatty — it's example code, and you'll be
 * lifting bits of it into your own project.
 */

declare(strict_types=1);

namespace ExtInferExample;

use Displace\Infer\InferException;
use Displace\Infer\Model;
use Displace\Infer\Prompt;
use Displace\Infer\Response;
use Symfony\Component\Console\Attribute\AsCommand;
use Symfony\Component\Console\Command\Command;
use Symfony\Component\Console\Input\InputArgument;
use Symfony\Component\Console\Input\InputInterface;
use Symfony\Component\Console\Input\InputOption;
use Symfony\Component\Console\Output\OutputInterface;
use Symfony\Component\Console\Style\SymfonyStyle;

#[AsCommand(
    name: 'chat',
    description: 'Multi-turn chat against a local GGUF model via ext-infer.',
)]
final class ChatCommand extends Command
{
    /** Default system prompt when the caller doesn't override it. */
    private const DEFAULT_SYSTEM = 'You are a helpful, concise assistant.';

    protected function configure(): void
    {
        $this
            ->addArgument(
                'model',
                InputArgument::REQUIRED,
                'Path to a GGUF model file (e.g. models/Qwen3-0.6B-Q8_0.gguf).',
            )
            ->addOption(
                'system',
                's',
                InputOption::VALUE_REQUIRED,
                'System prompt to seed the conversation.',
                self::DEFAULT_SYSTEM,
            )
            ->addOption(
                'max-tokens',
                'm',
                InputOption::VALUE_REQUIRED,
                'Maximum tokens generated per assistant turn.',
                '512',
            )
            ->addOption(
                'temperature',
                't',
                InputOption::VALUE_REQUIRED,
                'Sampling temperature (0.0 = greedy; ≥ 0.7 = more creative).',
                '0.7',
            );
    }

    protected function execute(InputInterface $input, OutputInterface $output): int
    {
        $io = new SymfonyStyle($input, $output);

        $modelPath   = (string) $input->getArgument('model');
        $system      = (string) $input->getOption('system');
        $maxTokens   = (int) $input->getOption('max-tokens');
        $temperature = (float) $input->getOption('temperature');

        if (!is_file($modelPath)) {
            $io->error("model file not found: {$modelPath}");
            return Command::INVALID;
        }

        $io->title('ext-infer interactive chat');
        $io->writeln([
            sprintf('<info>model</info>       : %s', $modelPath),
            sprintf('<info>system</info>      : %s', $system),
            sprintf('<info>max-tokens</info>  : %d', $maxTokens),
            sprintf('<info>temperature</info> : %.2f', $temperature),
            '',
            'Commands:',
            '  <comment>/reset</comment> — clear conversation history, keep system prompt',
            '  <comment>/show</comment>  — dump the current conversation in role/content form',
            '  <comment>/exit</comment>  — quit',
            '',
            'Pass <comment>-v</comment> to print the model\'s &lt;think&gt; reasoning alongside its answer.',
            '',
        ]);

        // Load the model. We keep one `Model` for the entire session; loading
        // is the slow part and we don't want to repeat it per turn.
        try {
            $model = Model::load($modelPath);
        } catch (InferException $e) {
            $io->error($e->getMessage());
            return Command::FAILURE;
        }

        // `Prompt` is immutable. `$base` is the empty conversation (system
        // message only), kept around so `/reset` can drop history without
        // re-loading the model.
        $base         = Prompt::system($system);
        $conversation = $base;
        $exitCode     = Command::SUCCESS;

        while (true) {
            $line = $io->ask('>');
            if ($line === null) {
                // EOF (^D). Treat as graceful exit.
                $io->writeln('');
                break;
            }
            $line = trim($line);
            if ($line === '') {
                continue;
            }

            if ($line === '/exit') {
                break;
            }
            if ($line === '/reset') {
                $conversation = $base;
                $io->note('conversation cleared');
                continue;
            }
            if ($line === '/show') {
                $this->renderConversation($conversation, $io);
                continue;
            }

            // Append the new user turn and ask the model to respond. Each
            // `with*` call returns a new Prompt instance; reassigning the
            // local is the idiomatic shape.
            $conversation = $conversation->withUser($line);

            try {
                $response = $model->chat(
                    $conversation,
                    maxTokens: $maxTokens,
                    temperature: $temperature,
                );
            } catch (InferException $e) {
                // Roll back the user turn so /show stays consistent with what
                // the model actually saw.
                $conversation = $conversation;
                $io->error("inference failed: " . $e->getMessage());
                continue;
            }

            $this->renderResponse($response, $output, $io);

            // Record the assistant turn for the next round. We store the
            // stripped answer rather than `text()` so the model isn't fed
            // its own `<think>` blocks back as conversation history —
            // doing so derails most reasoning models pretty quickly.
            $conversation = $conversation->withAssistant($response->answer());
        }

        $model->close();
        $io->success('bye.');
        return $exitCode;
    }

    /**
     * Print a `Response` to the user. Reasoning, if present, goes to a dim
     * line above the answer; with `-v` it's expanded.
     */
    private function renderResponse(
        Response $response,
        OutputInterface $output,
        SymfonyStyle $io,
    ): void {
        if ($response->hasReasoning()) {
            if ($output->isVerbose()) {
                $io->writeln('<fg=gray>--- reasoning ---</>');
                $io->writeln('<fg=gray>' . $response->reasoning() . '</>');
                $io->writeln('<fg=gray>--- /reasoning ---</>');
            } else {
                $io->writeln(sprintf(
                    '<fg=gray>[thought through %d tokens; pass -v to see it]</>',
                    $response->tokensGenerated(),
                ));
            }
        }

        $io->writeln('<info>' . $response->answer() . '</info>');

        if ($response->finishReason() === 'length') {
            $io->writeln(
                '<comment>(truncated — bump --max-tokens to see more)</comment>',
            );
        }
        $io->writeln('');
    }

    /**
     * Dump the conversation so far. Useful when the user wants to see how
     * many turns are in flight or has lost track of context.
     */
    private function renderConversation(Prompt $conversation, SymfonyStyle $io): void
    {
        $rows = [];
        foreach ($conversation->messages() as $msg) {
            $rows[] = [
                $msg->role(),
                strlen($msg->content()) > 80
                    ? substr($msg->content(), 0, 77) . '...'
                    : $msg->content(),
            ];
        }
        $io->table(['role', 'content'], $rows);
    }
}
