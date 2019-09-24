import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import Cookies from 'js-cookie';
import { BrowserRouter as Router, Route, Switch, Redirect } from 'react-router-dom';


export default class LoginScreen extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isAuthLoaded: false,
            authData: null
        };
    }
    scope = "none"
    code = "none"

    render() {
        if (!this.state.isAuthLoaded) {
            return (
                <div>Login Screen:
              <form onSubmit={(event) => {
                        event.preventDefault();
                        AppState.login(this, this.scope, this.code)
                    }}>
                        <p>scope:</p>
                        <input
                            type='text'
                            onChange={(e) => this.scope = event.target.value}
                        />
                        <p>code:</p>
                        <input
                            type='text'
                            onChange={(e) => this.code = event.target.value}
                        />
                        <input
                            type='submit'
                        />
                    </form>

                </div>
            )
        } else {
            return <Redirect to="/"/>
        }
    }
    
};
