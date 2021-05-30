#![allow(unused_imports)]
#![allow(clippy::write_with_newline)]
#![allow(clippy::len_zero)]

use std::convert::TryInto;
use lazy_static::lazy_static;
use rand::*;
use serenity::{
  client::{bridge::gateway::ShardManager, *},
  framework::standard::{macros::*, *},
  model::{channel::*, event::*, gateway::*, id::*, user::User},
  prelude::*,
  utils::*,
};
use std::{
  collections::{HashMap, HashSet},
  fmt::Write,
  process::{Command, Stdio},
  sync::Arc,
};
extern crate regex;
use regex::*;

lazy_static! {
    static ref DICE_MASSAGE: Regex = Regex::new(r"\s+(\+|-)").unwrap();
    static ref IMPLICIT_ROLL: Regex = Regex::new(r"^\s*(?:(\d*)((?:\+|-)?\d+)??)?$").unwrap();
    static ref DEFAULT_DICE: u32 = match ::std::env::var("DEFAULT_DICE") {
        Ok(num) => match num.parse::<u32>() {
          Ok(realnum) => realnum,
          Err(why) => panic!("DEFAULT_DICE environment variable is not a number. {:?}", why),
        },
        Err(_why2) => 20
      };
    static ref D10_ONLY_CHANNELS: Vec<u64> = match ::std::env::var("D10_ONLY_ROLLS") {
      Ok(rolls_str) => {
        let mut v : Vec<u64> = Vec::new();
        for channel in rolls_str.split_whitespace() {
          v.push(channel.to_string().parse::<u64>().unwrap());
        }
        v
      },
      Err(_) => vec![]
    };
}

#[group]
#[commands(dice, ten)]
pub struct General;

fn dice_get_string(author: &User, args: &str, ten: bool, channel_id: &ChannelId) -> String {
    let mut args_not_lower : String = args.to_string();
    let must_roll_tens = "ERROR: You must roll a d10 dice in this channel, either using the ?ten command or ?roll 1d10.";
    let mut invalid_sides = false;
    //println!("{}", args);
    if IMPLICIT_ROLL.is_match(args) {
      let caps = IMPLICIT_ROLL.captures(args).unwrap(); 
      let mut dd : u32 = *DEFAULT_DICE;
      if ten {
        dd = 10;
      }
      let mut first_num = match caps.get(1) {
        Some(cap) => cap.as_str(),
        None => "1"
      };
      let mut first_num_num : u32 = match first_num.parse::<u32>() {
        Ok(p) => {
          if p < 1 {
            first_num = "1";
            1
          }
          else {
            p
          }
        },
        Err(_) => {
          first_num = "1";
          1
        }
      };
      let plus_num = match caps.get(2) {
        Some(cap) => cap.as_str(),
        None => ""
      };
      //println!("first_num: {}, first_num_num: {}, plus_num: {}", first_num, first_num_num, plus_num);
      if first_num_num > 1 {
        let temparg = format!("1d{}{} ", dd, plus_num);
        if first_num_num > 50 {
          first_num_num = 50;
        }
        args_not_lower = temparg.repeat(first_num_num.try_into().unwrap()).trim().to_string();
      }
      else {
        args_not_lower = format!("{}d{}{}", first_num, dd, plus_num);
      }
    }
    let argslower = DICE_MASSAGE.replace_all(&args_not_lower.to_lowercase(), "$1").to_string();
    let mut output;
    let mut mb = MessageBuilder::new();
    let mut parsed_string;
    let mut first_iter = true;
    'exprloop: for dice_expression_str in argslower.split_whitespace().take(50) {
      let mut vec = Vec::new();
      let plus_only_form = dice_expression_str.replace("-", "+-");
      let mut total: i32 = 0;
      let mut sub_expressions = vec![];
      for sub_expression in plus_only_form.split('+').take(70) {
        if sub_expression.len() == 0 {
          continue;
        }
        let mut d_iter = sub_expression.split('d');
        let num_dice: i32 = match d_iter.next() {
          Some(num_dice_str) => {
            if num_dice_str.len() > 0 {
              match num_dice_str.parse::<i32>() {
                Ok(num) => num.max(-5_000).min(5_000),
                Err(_) => {
                  //msg.react(ReactionType::Unicode(EMOJI_QUESTION.to_string())).ok();
                  continue 'exprloop;
                }
              }
            } else {
              1
            }
          }
          None => {
            //msg.react(ReactionType::Unicode(EMOJI_QUESTION.to_string())).ok();
            continue 'exprloop;
          }
        };
        let num_sides: u32 = match d_iter.next() {
          Some(num_sides_str) => {
            match num_sides_str.parse::<u32>() {
              Ok(num) => num.min(4_000_000),
              Err(_) => {
                //msg.react(ReactionType::Unicode(EMOJI_QUESTION.to_string())).ok();
                continue 'exprloop;
              }
            }
          }
          None => 1,
        };

        if d_iter.next().is_some() {
          //msg.react(ReactionType::Unicode(EMOJI_QUESTION.to_string())).ok();
          continue 'exprloop;
        }
        if num_sides == 0 {
          // do nothing with 0-sided dice
        } else if num_sides == 1 {
          vec.push(num_dice);
          total += num_dice;
          sub_expressions.push(format!("{}", num_dice));
        } else {
          if D10_ONLY_CHANNELS.contains(channel_id.as_u64()) && num_sides != 10 {
            invalid_sides = true;
            break 'exprloop;
          }
          if num_dice > 0 {
            for _ in 0..num_dice {
              let pf: i32 = thread_rng().gen_range(1..=num_sides).try_into().unwrap();
              vec.push(pf);
              total += pf;
            }
            sub_expressions.push(format!("{}d{}", num_dice, num_sides));
          } else if num_dice < 0 {
            for _ in 0..num_dice.abs() {
              let pq: i32 = thread_rng().gen_range(1..=num_sides).try_into().unwrap();
              vec.push(pq);
              total -= pq;
            }
            sub_expressions.push(format!("{}d{}", num_dice, num_sides));
          }
          // do nothing if num_dice == 0
        }
      }
      if sub_expressions.len() > 0 {
        parsed_string = sub_expressions[0].clone();
        for sub_expression in sub_expressions.into_iter().skip(1) {
          if sub_expression.chars().nth(0) == Some('-') {
            parsed_string.push_str(&sub_expression);
          } else {
            parsed_string.push('+');
            parsed_string.push_str(&sub_expression);
          }
        }
        let veq = format!("{:?}", vec).replace("[", "(").replace("]", ")");
        if first_iter {
          mb.mention(author)
          .push(" requested ")
          .push(&args_not_lower)
          .push( " and rolled ")
          .push_bold(total);
        }
        else {
          mb.push(", ")
          .push_bold(total);
        }
        
        if vec.len() > 1 {
          mb.push(" ")
          .push(veq);
        }
      } 
      first_iter = false;
    }
    if mb.build().len() > 0 {
      mb.push(".");
    }
    output = mb.build();
    if output.len() <= 0 {
      mb = MessageBuilder::new();
      mb.mention(author)
      .push(" Unable to process the supplied dice expression because I didn't understand the dice syntax you supplied.");
      output = mb.build();
    }
    if invalid_sides == true {
      mb = MessageBuilder::new();
      mb.mention(author)
      .push(must_roll_tens);
      output = mb.build();
    }
    return output;
  }
  
  #[command]
  #[aliases("roll", "dice")]
  #[description = "Rolls a standard dice expression"]
  #[usage = "EXPRESSION [...]"]
  pub async fn dice(_ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut output = dice_get_string(&msg.author, args.rest(), false, &msg.channel_id);
  
    let yelling = msg.content.contains("ROLL");
    if yelling {
      output = "Wow, okay! Is your caps lock on, or are you mad at me? :( ".to_owned() + &output;
    }
  
    if let Err(why) = msg.channel_id.say(&_ctx.http, output).await {
      println!("Error sending message: {:?}", why);
      let built : String = "ERROR: Failed to send you a valid response, either because the response would be too long, or the Discord server didn't like it for some other reason. Please try again.".to_string();
      if let Err(why2) = msg.channel_id.say(&_ctx.http, built).await {
        println!("Error sending message: {:?}", why2);
      }
    }
    Ok(())
  }
  
  #[command]
  #[aliases("ten")]
  #[description = "Rolls a standard dice expression assuming d10"]
  #[usage = "EXPRESSION [...]"]
  pub async fn ten(_ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mut output = dice_get_string(&msg.author, args.rest(), true, &msg.channel_id);
  
    let yelling = msg.content.contains("TEN");
    if yelling {
      output = "Wow, okay! Is your caps lock on, or are you mad at me? :( ".to_owned() + &output;
    }
  
    if let Err(why) = msg.channel_id.say(&_ctx.http, output).await {
      println!("Error sending message: {:?}", why);
      let built : String = "ERROR: Failed to send you a valid response, either because the response would be too long, or the Discord server didn't like it for some other reason. Please try again.".to_string();
      if let Err(why2) = msg.channel_id.say(&_ctx.http, built).await {
        println!("Error sending message: {:?}", why2);
      }
    }
    Ok(())
  }
  
//   #[cfg(test)]
//   mod tests {
//     use super::*;
  
//     fn rng0() -> PCG32 {
//       return PCG32::seed(0,0);
//     }
  
//     fn test_get_num(index: u32, die: u32) -> u32 {
//       let mut rng = rng0();
//       let rr = RandRangeU32::new(1, die);
//       let mut retval : u32 = 0;
//       for _i in 0..index+1 {
//         retval = rr.sample(&mut rng);
//       }
//       return retval;
//     }
    
//     #[test]
//     fn dice_str_nonsense() {
//       let gen = &mut rng0();
//       let user : User = User::default();
//       let args = " Unable to process the supplied dice expression because I didn't understand the dice syntax you supplied.";
//       assert_eq!(dice_get_string(gen, &user, args, false), "<@210> Unable to process the supplied dice expression because I didn't understand the dice syntax you supplied.");
//     }
    
//     #[test]
//     fn dice_str_torture() {
//       let tests: HashMap<&str, String> = 
//       [
//         ("1d10", format!("**{}**", test_get_num(0, 10))),
//         ("1d10+1", format!("**{}** ({}, {})", test_get_num(0, 10)+1, test_get_num(0, 10), 1)),
//         ("1d10+1 1D10", format!("**{}** ({}, {}), **{}**", test_get_num(0, 10)+1, test_get_num(0, 10), 1, test_get_num(1, 10))),
//         ("1d20+1", format!("**{}** ({}, {})", test_get_num(0, 20)+1, test_get_num(0, 20), 1)),
//         ("1d20+2 1d20+3", format!("**{}** ({}, {}), **{}** ({}, {})", test_get_num(0, 20)+2, test_get_num(0, 20), 2, test_get_num(1, 20)+3, test_get_num(1, 20), 3)),
//         ("1d20 +2 1d20  +3", format!("**{}** ({}, {}), **{}** ({}, {})", test_get_num(0, 20)+2, test_get_num(0, 20), 2, test_get_num(1, 20)+3, test_get_num(1, 20), 3)),
//         ("1D20 +2 1D20  +3", format!("**{}** ({}, {}), **{}** ({}, {})", test_get_num(0, 20)+2, test_get_num(0, 20), 2, test_get_num(1, 20)+3, test_get_num(1, 20), 3)),
//       ]
//       .iter().cloned().collect();
//       let user : User = User::default();
//       for (args, retnum) in tests {
//         let retval = format!("<@{}> requested {} and rolled {}.", user.id, args, retnum);
//         let gen = &mut rng0();
//         assert_eq!(dice_get_string(gen, &user, args, false), retval);
//       }
//     }
  
//     #[test]
//     fn dice_str_default() {
//       let roll1 : u32 = test_get_num(0, *DEFAULT_DICE);
//       let roll2 : u32 = test_get_num(1, *DEFAULT_DICE);
//       let gen = &mut rng0();
//       let user : User = User::default();
//       let mut retval = format!("<@{}> requested {} and rolled **{}** ({}, {}).", user.id, "1d20+2", roll1+2, roll1, 2);
//       assert_eq!(dice_get_string(&mut rng0(), &user, "+2", false), retval);
  
//       retval = format!("<@{}> requested {} and rolled **{}**.", user.id, "1d20", roll1);
//       assert_eq!(dice_get_string(&mut rng0(), &user, "", false), retval);
  
//       retval = format!("<@{}> requested {} and rolled **{}**, **{}**.", user.id, "1d20 1d20", roll1, roll2);
//       assert_eq!(dice_get_string(&mut rng0(), &user, "2", false), retval);
//     }
  
//     #[test]
//     fn ten_default() {
//       let roll1 : u32 = test_get_num(0, 10);
//         let roll2 : u32 = test_get_num(1, 10);
//         let gen = &mut rng0();
//         let user : User = User::default();
//         let mut retval = format!("<@{}> requested {} and rolled **{}** ({}, {}).", user.id, "1d10+2", roll1+2, roll1, 2);
//         assert_eq!(dice_get_string(gen, &user, "+2", true), retval);
//         retval = format!("<@{}> requested {} and rolled **{}**.", user.id, "1d10", roll2);
//         assert_eq!(dice_get_string(gen, &user, "", true), retval);
//     }
//   }
  