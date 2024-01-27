#!/usr/bin/env node
import 'source-map-support/register';
import * as cdk from 'aws-cdk-lib';
import { Zero2ProdApplicationStack } from '../lib/application-stack';
import { getParameter } from '@aws-lambda-powertools/parameters/ssm';

const app = new cdk.App();
new Zero2ProdApplicationStack(app, 'Zero2ProdApplicationStack', {
});